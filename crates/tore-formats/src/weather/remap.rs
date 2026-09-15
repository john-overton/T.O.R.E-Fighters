//! Source LAY shade headers and indexed haze tables (FA 0x4b3ad0/0x4b3410).
use crate::{Result, invalid, slice, u32_at};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShadeRemap {
    pub color: [u8; 3],
    pub levels: Vec<[u8; 256]>,
}

impl ShadeRemap {
    /// Density 256 saturates at the last table rather than indexing past it.
    pub fn level(&self, density: i32) -> &[u8; 256] {
        let index =
            ((density.clamp(0, 256) as usize * self.levels.len()) >> 8).min(self.levels.len() - 1);
        &self.levels[index]
    }
}

pub(super) fn parse(code: &[u8], base: usize, table: usize) -> Result<Vec<ShadeRemap>> {
    let read = |rva: usize, size| {
        slice(
            code,
            rva.checked_sub(base)
                .ok_or_else(|| invalid("shade RVA before CODE"))?,
            size,
        )
    };
    let mut shades = Vec::new();
    for index in 0..32 {
        let address = table
            .checked_add(index * 48)
            .ok_or_else(|| invalid("shade address overflow"))?;
        if read(address, 1)?[0] != 0 {
            if shades.is_empty() {
                return Err(invalid("empty weather shade table"));
            }
            return Ok(shades);
        }
        let header = read(address, 48)?;
        let color: [u8; 3] = header[1..4].try_into().unwrap();
        let count = u32_at(header, 4)?;
        if color.iter().any(|c| *c > 63) || !(1..=10).contains(&count) {
            return Err(invalid("unsupported weather shade header"));
        }
        let mut levels = Vec::new();
        for i in 0..count {
            levels.push(read(u32_at(header, 8 + i * 4)?, 256)?.try_into().unwrap());
        }
        shades.push(ShadeRemap { color, levels });
    }
    Err(invalid("weather shade table has no bounded sentinel"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_tables_and_density_edges() {
        let module = super::super::Module::parse(&super::super::synthetic_module(1)).unwrap();
        let mut shade = module.shades[0].clone();
        for (i, level) in shade.levels.iter_mut().enumerate() {
            level.fill(i as u8);
        }
        for (density, expected) in [
            (-1, 0),
            (0, 0),
            (25, 0),
            (26, 1),
            (255, 9),
            (256, 9),
            (1000, 9),
        ] {
            assert_eq!(shade.level(density)[123], expected);
        }
        let data = super::super::synthetic_module(1);
        let (code, base) = super::super::section(&data).unwrap();
        for size in [0, 0x400, 0x42f, 0x5ff] {
            assert!(parse(&code[..size], base, 0x500).is_err());
        }
        let mut bad = code.to_vec();
        bad[0x404..0x408].copy_from_slice(&11u32.to_le_bytes());
        assert!(parse(&bad, base, 0x500).is_err());
        bad[0x400] = 1;
        assert!(parse(&bad, base, 0x500).is_err());
    }
}
