//! Bounded inert FA lens-flare descriptors; no native code is executed.
use crate::{Result, invalid, slice, u16_at, u32_at};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Circle {
    pub offset_percent: i16,
    pub radius: u16,
    pub fill: u16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub circles: Vec<Circle>,
}
impl Layout {
    /// Read the descriptors from a reviewed build, selected by fingerprint.
    pub fn parse(exe: &[u8]) -> Result<Self> {
        Self::parse_with(crate::executable::identify(exe)?, exe)
    }

    /// Read the descriptors using an already chosen build address set.
    pub fn parse_with(build: &crate::executable::Layout, exe: &[u8]) -> Result<Self> {
        let image = crate::ui::creator::Image::parse(exe)?;
        let mut circles = Vec::new();
        for i in 0..17 {
            let r = image.read(build.flare_records + i * 12, 12, false)?;
            let radius = u32_at(r, 4)?;
            if radius == 0 {
                let out = Self { circles };
                out.validate()?;
                return Ok(out);
            }
            let offset = u32_at(r, 0)? as i32;
            let fill = u32_at(r, 8)?;
            if !(-1024..=1024).contains(&offset) || radius > 256 || ![265, 266].contains(&fill) {
                return Err(invalid("unsupported flare descriptor"));
            }
            circles.push(Circle {
                offset_percent: offset as i16,
                radius: radius as u16,
                fill: fill as u16,
            });
        }
        Err(invalid("flare sentinel missing"))
    }
    pub fn validate(&self) -> Result<()> {
        if self.circles.is_empty()
            || self.circles.len() > 16
            || self.circles.iter().any(|c| {
                !(-1024..=1024).contains(&c.offset_percent)
                    || !(1..=256).contains(&c.radius)
                    || ![265, 266].contains(&c.fill)
            })
        {
            return Err(invalid("invalid flare layout"));
        }
        Ok(())
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut b = b"TOREFLR1".to_vec();
        b.push(self.circles.len() as u8);
        for c in &self.circles {
            b.extend(c.offset_percent.to_le_bytes());
            b.extend(c.radius.to_le_bytes());
            b.extend(c.fill.to_le_bytes());
        }
        b
    }
    pub fn decode(b: &[u8]) -> Result<Self> {
        if slice(b, 0, 8)? != b"TOREFLR1" {
            return Err(invalid("flare cache version"));
        }
        let n = usize::from(slice(b, 8, 1)?[0]);
        if b.len() != 9 + 6 * n {
            return Err(invalid("flare cache length"));
        }
        let mut circles = Vec::new();
        for c in b[9..].chunks_exact(6) {
            circles.push(Circle {
                offset_percent: u16_at(c, 0)? as i16,
                radius: u16_at(c, 2)? as u16,
                fill: u16_at(c, 4)? as u16,
            });
        }
        let out = Self { circles };
        out.validate()?;
        Ok(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_synthetic_flare_cache() {
        let layout = Layout {
            circles: vec![Circle {
                offset_percent: -25,
                radius: 3,
                fill: 265,
            }],
        };
        let bytes = layout.encode();
        assert_eq!(Layout::decode(&bytes).unwrap(), layout);
        for n in 0..bytes.len() {
            assert!(Layout::decode(&bytes[..n]).is_err());
        }
        assert!(Layout::parse(&bytes).is_err());
        let mut bad = bytes;
        bad[13] = 0;
        assert!(Layout::decode(&bad).is_err());
    }

    /// The same nine synthetic descriptors at each reviewed build's addresses.
    fn image(build: &crate::executable::Layout) -> Vec<u8> {
        let mut data = vec![0u8; 12 * 10];
        for i in 0..9usize {
            let r = i * 12;
            let offset = (i as i32 * 100 - 400) as u32;
            for (at, value) in [(0, offset), (4, i as u32 + 1), (8, 265 + (i as u32 & 1))] {
                data[r + at..r + at + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        crate::executable::fixture(&[(".data", build.flare_records, data, false)])
    }

    #[test]
    fn both_reviewed_builds_decode_the_same_circles() {
        let [disc, patch] = crate::executable::LAYOUTS;
        let a = Layout::parse_with(&disc, &image(&disc)).unwrap();
        assert_eq!(a, Layout::parse_with(&patch, &image(&patch)).unwrap());
        assert_eq!(a.circles.len(), 9);
        assert_eq!(a.circles[0].offset_percent, -400);
        assert!(Layout::parse(&image(&patch)).is_err());
    }
}
