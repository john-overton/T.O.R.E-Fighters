//! The retail game's text character set.
//!
//! Imported text is 8-bit DOS text, code page 437: the original fonts draw
//! exactly the CP437 letters (accented vowels at 0x80 to 0xA5, the inverted
//! marks at 0xA8 and 0xAD, the sharp s at 0xE1, the micro sign at 0xE6 and the
//! degree sign at 0xF8) and nothing else above 0x7F that is text. In the
//! imported data the only byte above 0x7F in any text is 0x89, the e with a
//! diaeresis in the Kurile airport "Berezovka". The app holds text as Unicode
//! and draws it through [`glyph_codes`], which puts each character back on the
//! font's CP437 cell.

/// CP437 0x80 to 0xFF as Unicode.
const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ',
    'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ',
    'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕',
    '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦',
    '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐',
    '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±',
    '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
];

/// Decodes retail 8-bit text (ASCII, or CP437 above 0x7F) without loss.
pub fn decode_cp437(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| {
            if *b < 0x80 {
                char::from(*b)
            } else {
                CP437_HIGH[usize::from(*b - 0x80)]
            }
        })
        .collect()
}

/// The CP437 code of a character, or `?` when the character has none.
pub fn cp437_code(c: char) -> u8 {
    if c.is_ascii() {
        return c as u8;
    }
    CP437_HIGH
        .iter()
        .position(|h| *h == c)
        .map_or(b'?', |i| 0x80 + i as u8)
}

/// True for a character the original fonts draw: printable ASCII, or one of
/// the CP437 letters and marks above 0x7F.
pub fn is_drawn(c: char) -> bool {
    c == ' '
        || c.is_ascii_graphic()
        || matches!(cp437_code(c), 0x80..=0x9D | 0x9F..=0xA8 | 0xAD | 0xE1 | 0xE6 | 0xF8)
}

/// The font cell of each character of `text`.
pub fn glyph_codes(text: &str) -> impl Iterator<Item = u8> + '_ {
    text.chars().map(cp437_code)
}

/// `str::glyph_codes()`, for drawing code that walks a string's font cells.
pub trait GlyphCodes {
    fn glyph_codes(&self) -> impl Iterator<Item = u8> + '_;
}
impl GlyphCodes for str {
    fn glyph_codes(&self) -> impl Iterator<Item = u8> + '_ {
        glyph_codes(self)
    }
}

/// Upper case that never leaves the character set: a letter whose capital has
/// no CP437 cell (such as e with a diaeresis) stays as it is.
pub fn upper(text: &str) -> String {
    text.chars()
        .map(|c| {
            let mut up = c.to_uppercase();
            match (up.next(), up.next()) {
                (Some(u), None) if u.is_ascii() || cp437_code(u) != b'?' => u,
                _ => c,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cp437_round_trips_and_the_kurile_airport_reads_right() {
        assert_eq!(decode_cp437(b"Ber\x89zovka"), "Ber\u{eb}zovka");
        for code in 0..=255u8 {
            let text = decode_cp437(&[code]);
            assert_eq!(cp437_code(text.chars().next().unwrap()), code);
        }
        assert_eq!(cp437_code('\u{2603}'), b'?');
        assert_eq!(glyph_codes("Ber\u{eb}zovka").nth(3), Some(0x89));
    }
    #[test]
    fn the_drawn_set_matches_the_original_fonts() {
        assert!(is_drawn('a') && is_drawn(' ') && is_drawn('\u{eb}') && is_drawn('\u{df}'));
        assert!(!is_drawn('\u{2500}') && !is_drawn('\n') && !is_drawn('\u{20a7}'));
    }
    #[test]
    fn upper_case_stays_inside_the_character_set() {
        assert_eq!(upper("Ber\u{eb}zovka"), "BER\u{eb}ZOVKA");
        assert_eq!(upper("\u{e9}cole"), "\u{c9}COLE");
        assert_eq!(upper("abc"), "ABC");
    }
    #[test]
    fn no_decoded_character_is_the_replacement_character() {
        let all: Vec<u8> = (0..=255).collect();
        assert!(!decode_cp437(&all).contains('\u{fffd}'));
    }
}
