//! Reviewed HUD module data only; native code and unreviewed fields are inert.
use crate::{Result, invalid, module, slice};

/// The name field of the instrument window frame picture: 13 bytes at root
/// +0x275, ending before the next name at +0x282.
const PANEL: std::ops::Range<usize> = 0x275..0x282;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hud {
    pub primary_color: u8,
    /// Instrument window frame picture, as stored (for example `~f4_p`).
    /// None when the field is empty.
    pub panel: Option<String>,
    /// Cockpit palette index for the window title and number (+0x2a8).
    pub title_color: u8,
    /// Cockpit palette index for the window button letters (+0x2a9).
    pub button_color: u8,
    /// Cockpit palette index for the square shown on a button press (+0x2aa).
    pub press_color: u8,
}
impl Hud {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let (code, _) = module::code(bytes)?;
        // FA 0x406193 copies 0x2b2 bytes to 0x521360. _HUDDraw reads +0x72.
        let root = slice(code, 0, 0x2b2)?;
        let field = &root[PANEL];
        let end = field
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| invalid("unterminated HUD panel name"))?;
        let name = &field[..end];
        if !name.iter().all(u8::is_ascii_graphic) {
            return Err(invalid("unreviewed HUD panel name bytes"));
        }
        Ok(Self {
            primary_color: root[0x72],
            panel: (!name.is_empty()).then(|| String::from_utf8_lossy(name).into_owned()),
            title_color: root[0x2a8],
            button_color: root[0x2a9],
            press_color: root[0x2aa],
        })
    }
    /// The archive resource holding the frame picture, for example `~F4_P.PIC`.
    pub fn panel_resource(&self) -> Option<String> {
        self.panel
            .as_ref()
            .map(|name| format!("{}.PIC", name.to_ascii_uppercase()))
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
    #[test]
    fn panel_name_and_window_text_colors_come_from_the_root() {
        let mut code = vec![0; 0x2b2];
        code[0x275..0x27a].copy_from_slice(b"~ab_p");
        // The neighbouring name field must not leak into the panel name.
        code[0x282..0x287].copy_from_slice(b"~ab_w");
        code[0x2a8] = 39;
        code[0x2a9] = 19;
        code[0x2aa] = 4;
        let hud = Hud::parse(&module::fixture(&code)).unwrap();
        assert_eq!(hud.panel.as_deref(), Some("~ab_p"));
        assert_eq!(hud.panel_resource().as_deref(), Some("~AB_P.PIC"));
        assert_eq!(
            (hud.title_color, hud.button_color, hud.press_color),
            (39, 19, 4)
        );
    }
    #[test]
    fn panel_name_is_bounded_terminated_and_optional() {
        let mut code = vec![0; 0x2b2];
        let empty = Hud::parse(&module::fixture(&code)).unwrap();
        assert_eq!((empty.panel.clone(), empty.panel_resource()), (None, None));
        // Twelve characters and the terminator fill the field exactly.
        code[0x275..0x281].copy_from_slice(b"~abcdefghi_p");
        let full = Hud::parse(&module::fixture(&code)).unwrap();
        assert_eq!(full.panel.as_deref(), Some("~abcdefghi_p"));
        // Thirteen characters would run into the next field.
        code[0x281] = b'x';
        assert!(Hud::parse(&module::fixture(&code)).is_err());
        code[0x281] = 0;
        code[0x276] = 0x7f;
        assert!(Hud::parse(&module::fixture(&code)).is_err());
    }
}
