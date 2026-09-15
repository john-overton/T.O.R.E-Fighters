//! Original per-normal indexed light maps, FA 0x4cd854 / 0x4cc4b4.
use crate::{Result, invalid, slice, u32_at};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lighting {
    pub shade: Vec<[u8; 256]>,
    pub highlight: Vec<[u8; 256]>,
}
impl Lighting {
    pub(super) fn parse(code: &[u8], base: usize) -> Result<Self> {
        let read = |count_at, pointers| -> Result<Vec<[u8; 256]>> {
            let count = u32_at(code, count_at)?;
            if !(1..=10).contains(&count) {
                return Err(invalid("invalid light remap count"));
            }
            (0..count)
                .map(|i| {
                    let at = u32_at(code, pointers + i * 4)?
                        .checked_sub(base)
                        .ok_or_else(|| invalid("light remap before CODE"))?;
                    Ok(slice(code, at, 256)?.try_into().unwrap())
                })
                .collect()
        };
        Ok(Self {
            shade: read(0x14, 0x18)?,
            highlight: read(0x40, 0x44)?,
        })
    }
    /// Returns bank (0 shade / 1 highlight) and row. Source amount is -255..255;
    /// negative highlights are enabled for the reviewed object rendering path.
    pub fn row(&self, amount: i16) -> (usize, usize) {
        let amount = i32::from(amount).clamp(-255, 255);
        if amount < -192 {
            (1, ((-amount - 192) as usize * self.highlight.len()) >> 6)
        } else {
            (0, (amount.max(0) as usize * self.shade.len()) >> 8)
        }
    }
}
/// Signed shifts happen on each product before summation, then on the total.
/// Caller supplies bounded Q15 vectors; orientation rounding belongs to caller.
pub fn amount(normal: [i16; 3], direction: [i16; 3]) -> i16 {
    let dot: i32 = normal
        .into_iter()
        .zip(direction)
        .map(|(a, b)| (i32::from(a) * i32::from(b)) >> 15)
        .sum();
    (dot >> 7).clamp(-255, 255) as i16
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn light_tables_reject_invalid_counts_and_pointers() {
        let mut code = vec![0; 512];
        for (count, pointer) in [(0x14, 0x18), (0x40, 0x44)] {
            code[count..count + 4].copy_from_slice(&1u32.to_le_bytes());
            code[pointer..pointer + 4].copy_from_slice(&256u32.to_le_bytes());
        }
        assert!(Lighting::parse(&code, 0).is_ok());
        assert!(Lighting::parse(&code[..511], 0).is_err());
        assert!(Lighting::parse(&code, 257).is_err());
        code[0x14] = 11;
        assert!(Lighting::parse(&code, 0).is_err());
        code[0x14] = 0;
        assert!(Lighting::parse(&code, 0).is_err());
    }
    #[test]
    fn original_light_banks_and_signed_dot_boundaries() {
        let maps = Lighting {
            shade: vec![[0; 256]; 7],
            highlight: vec![[0; 256]; 6],
        };
        assert_eq!(maps.row(-255), (1, 5));
        assert_eq!(maps.row(-193), (1, 0));
        assert_eq!(maps.row(-192), (0, 0));
        assert_eq!(maps.row(0), (0, 0));
        assert_eq!(maps.row(36), (0, 0));
        assert_eq!(maps.row(37), (0, 1));
        assert_eq!(maps.row(255), (0, 6));
        assert_eq!(amount([32767, 0, 0], [32767, 0, 0]), 255);
        assert_eq!(amount([-32767, 0, 0], [32767, 0, 0]), -255);
        assert_eq!(amount([-1, 0, 0], [1, 0, 0]), -1);
    }
}
