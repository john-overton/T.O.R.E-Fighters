//! Inert FA cloud descriptor table and portable cache; never execute the image.
use crate::{Result, invalid, slice, u16_at, u32_at};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patch {
    pub mask: u8,
    pub x_f8: i32,
    pub z_f8: i32,
    pub yaw: i16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub patches: Vec<Patch>,
    pub period_f8: i32,
    pub subdivisions: u8,
}
impl Layout {
    /// Read the table from a reviewed build, selected by fingerprint.
    pub fn parse(exe: &[u8]) -> Result<Self> {
        Self::parse_with(crate::executable::identify(exe)?, exe)
    }

    /// Read the table using an already chosen build address set.
    pub fn parse_with(build: &crate::executable::Layout, exe: &[u8]) -> Result<Self> {
        let image = crate::ui::creator::Image::parse(exe)?;
        let code = image.read(build.cloud_call, 7, true)?;
        if code[0] != 0x6a || code[2] != 0x68 {
            return Err(invalid("cloud repeat call changed"));
        }
        let mut out = Self {
            patches: Vec::new(),
            period_f8: u32_at(code, 3)? as i32,
            subdivisions: code[1],
        };
        for i in 0..64 {
            let r = image.read(build.cloud_records + 26 * i, 26, false)?;
            let flags = u32_at(r, 0)?;
            if flags == 0 {
                out.validate()?;
                return Ok(out);
            }
            if image.read(u32_at(r, 4)?, 10, false)? != b"cloud1.SH\0"
                || u32_at(r, 8)? != 0
                || u32_at(r, 16)? != 0
                || flags > 255
            {
                return Err(invalid("unsupported cloud descriptor"));
            }
            out.patches.push(Patch {
                mask: flags as u8,
                x_f8: u32_at(r, 12)? as i32,
                z_f8: u32_at(r, 20)? as i32,
                yaw: u16_at(r, 24)? as i16,
            });
        }
        Err(invalid("cloud layout sentinel missing"))
    }
    pub fn validate(&self) -> Result<()> {
        if self.patches.is_empty()
            || self.patches.len() > 64
            || self.period_f8 <= 0
            || !(self.period_f8 as u32).is_power_of_two()
            || self.period_f8 > 1 << 27
            || self.subdivisions > 2
            || self.patches.iter().any(|p| {
                !(1..=3).contains(&p.mask)
                    || p.x_f8 < 0
                    || p.x_f8 >= self.period_f8
                    || p.z_f8 < 0
                    || p.z_f8 >= self.period_f8
            })
        {
            return Err(invalid("invalid cloud layout"));
        }
        Ok(())
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut b = b"TORECLO1".to_vec();
        b.extend(self.period_f8.to_le_bytes());
        b.push(self.subdivisions);
        b.push(self.patches.len() as u8);
        for p in &self.patches {
            b.push(p.mask);
            b.extend(p.x_f8.to_le_bytes());
            b.extend(p.z_f8.to_le_bytes());
            b.extend(p.yaw.to_le_bytes());
        }
        b
    }
    pub fn decode(b: &[u8]) -> Result<Self> {
        if slice(b, 0, 8)? != b"TORECLO1" {
            return Err(invalid("cloud cache version"));
        }
        let h = slice(b, 0, 14)?;
        let n = usize::from(h[13]);
        if b.len() != 14 + n * 11 {
            return Err(invalid("cloud cache length"));
        }
        let mut out = Self {
            period_f8: u32_at(b, 8)? as i32,
            subdivisions: h[12],
            patches: Vec::new(),
        };
        for r in b[14..].chunks_exact(11) {
            out.patches.push(Patch {
                mask: r[0],
                x_f8: u32_at(r, 1)? as i32,
                z_f8: u32_at(r, 5)? as i32,
                yaw: u16_at(r, 9)? as i16,
            });
        }
        out.validate()?;
        Ok(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_cache_bounds_and_provenance_gate() {
        let l = Layout {
            patches: vec![Patch {
                mask: 1,
                x_f8: 512,
                z_f8: 256,
                yaw: 90,
            }],
            period_f8: 1024,
            subdivisions: 2,
        };
        let b = l.encode();
        assert_eq!(Layout::decode(&b).unwrap(), l);
        for end in 0..b.len() {
            assert!(Layout::decode(&b[..end]).is_err());
        }
        let mut bad = b;
        bad[14] = 4;
        assert!(Layout::decode(&bad).is_err());
        assert!(Layout::parse(&bad).is_err());
    }

    /// The same nine synthetic patches at each reviewed build's addresses.
    fn image(build: &crate::executable::Layout) -> Vec<u8> {
        let mut code = vec![0x6a, 0x02, 0x68, 0, 0, 0, 0x02];
        code.resize(16, 0);
        let mut data = vec![0u8; 26 * 10 + 10];
        let name = build.cloud_records + 26 * 10;
        data[26 * 10..].copy_from_slice(b"cloud1.SH\0");
        for i in 0..9usize {
            let r = i * 26;
            for (at, value) in [
                (0, i % 3 + 1),
                (4, name),
                (12, 100 + i * 10),
                (20, 200 + i * 10),
            ] {
                data[r + at..r + at + 4].copy_from_slice(&(value as u32).to_le_bytes());
            }
            data[r + 24..r + 26].copy_from_slice(&(i as u16 * 7).to_le_bytes());
        }
        crate::executable::fixture(&[
            ("CODE", build.cloud_call, code, true),
            (".data", build.cloud_records, data, false),
        ])
    }

    #[test]
    fn both_reviewed_builds_decode_the_same_patches() {
        let [disc, patch] = crate::executable::LAYOUTS;
        let a = Layout::parse_with(&disc, &image(&disc)).unwrap();
        assert_eq!(a, Layout::parse_with(&patch, &image(&patch)).unwrap());
        assert_eq!(a.patches.len(), 9);
        assert_eq!(a.period_f8, 1 << 25);
        assert_eq!(a.subdivisions, 2);
        assert!(Layout::parse(&image(&patch)).is_err());
    }
}
