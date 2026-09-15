//! Reviewed HUD module data only; native code and unreviewed fields are inert.
use crate::{Result, module, slice};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hud {
    pub primary_color: u8,
}
impl Hud {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let (code, _) = module::code(bytes)?;
        // FA 0x406193 copies 0x2b2 bytes to 0x521360. _HUDDraw reads +0x72.
        let root = slice(code, 0, 0x2b2)?;
        Ok(Self {
            primary_color: root[0x72],
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn primary_color_comes_from_bounded_code_not_file_offset() {
        let mut code = vec![0; 0x2b2];
        code[0x72] = 42;
        let bytes = module::fixture(&code);
        assert_eq!(Hud::parse(&bytes).unwrap().primary_color, 42);
        assert!(Hud::parse(&module::fixture(&code[..0x2b1])).is_err());
        assert!(Hud::parse(&code).is_err());
    }
}
