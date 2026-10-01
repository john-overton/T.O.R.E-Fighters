//! The kit's pieces: every retail picture the widgets draw, decoded once.
use crate::AppResult;
use crate::menu::Sprite;
use std::collections::BTreeMap;
use tore_formats::Pic;

/// The two retail backgrounds a connection screen is made of. Each is decoded
/// in its own palette, whatever palette the screen's pieces use.
pub const BACKGROUNDS: [&str; 2] = ["MODEM3", "NETIPX3"];

/// Every picture the widgets draw besides the backgrounds, by retail name.
/// The rockers, button pieces, `PANELFNT`, `SMLFONT` and the button fonts
/// come from the menu art the game always decodes; the rest from the
/// multiplayer art (`Assets::multiplayer_resources`).
pub const PIECES: &[&str] = &[
    // The panel.
    "PANEL", "EDGETL", "EDGETR", "EDGEBL", "EDGEBR", "EDGETB", "EDGELR",
    // Buttons: green, grey, blue default, grey default, and the default caps.
    "ACTION0L", "ACTION0M", "ACTION0R", "ACTIOD0L", "ACTIOD0M", "ACTIOD0R", "ACTDFT0L", "ACTDFT0M",
    "ACTDFT0R", "ACTDFD0L", "ACTDFD0M", "ACTDFD0R", "ACTDFLT", "ACTDFLD",
    // Fonts: panel text, its dim copy, list text, button labels, typed text.
    "PANELFNT", "PANELFND", "SMLFONT", "FONTACT", "FONTACD", "FONTDFT", "FONTDFD", "WHEELFNT",
    // Lists and the page box.
    "LISTLFT", "LISTMID", "LISTRT", "LISTHI", "PAGEBOX", // Text fields.
    "EDITL", "EDITM", "EDITR", // Check boxes.
    "CHECK00", "CHECK01", "CHECK02", "CHECK03", "CHECK04", "CHECK05", "CHECK06",
    // The PREV/NEXT rocker.
    "ROCKER00", "ROCKER01", "ROCKER02", "ROCKER03", "ROCKER04",
];

/// The decoded pieces. Sprites are keyed `NAME.PIC`, as the menu's own sprite
/// maps are, so `Canvas::button_style` can draw from [`Kit::sprites`] as it is.
pub struct Kit {
    pub(crate) sprites: BTreeMap<String, Sprite>,
}

/// A picture found in one of the two maps.
enum Source<'a> {
    Parsed(Pic),
    Held(&'a Pic),
}
impl std::ops::Deref for Source<'_> {
    type Target = Pic;
    fn deref(&self) -> &Pic {
        match self {
            Source::Parsed(pic) => pic,
            Source::Held(pic) => pic,
        }
    }
}

/// The picture's own palette laid over black.
fn palette_of(pic: &Pic) -> [[u8; 3]; 256] {
    let mut palette = [[0u8; 3]; 256];
    let n = pic.palette.len().min(256);
    palette[..n].copy_from_slice(&pic.palette[..n]);
    palette
}

fn decode(pic: &Pic, palette: &[[u8; 3]; 256]) -> Sprite {
    Sprite {
        width: pic.width,
        height: pic.height,
        rgba: pic.rgba(palette),
        glyphs: pic.glyphs.clone(),
    }
}

impl Kit {
    /// Decodes the pieces in the palette of `primary` (one of
    /// [`BACKGROUNDS`]: the picture the screen is named after) and both
    /// backgrounds in their own. `pics` is the game's menu art and
    /// `multiplayer` the imported multiplayer pieces
    /// (`Assets::multiplayer_resources`); a missing piece names itself in the
    /// error and asks for a re-import.
    pub fn new(
        pics: &BTreeMap<String, Pic>,
        multiplayer: &BTreeMap<String, Vec<u8>>,
        primary: &str,
    ) -> AppResult<Self> {
        let find = |name: &str| -> AppResult<Source<'_>> {
            let file = format!("{name}.PIC");
            if let Some(bytes) = multiplayer.get(&file) {
                return Ok(Source::Parsed(
                    Pic::parse(bytes).map_err(|error| format!("{file}: {error}"))?,
                ));
            }
            pics.get(&file)
                .map(Source::Held)
                .ok_or_else(|| format!("multiplayer art missing {file}; re-import media").into())
        };
        let mut backgrounds = BTreeMap::new();
        for name in BACKGROUNDS {
            backgrounds.insert(name, find(name)?);
        }
        let primary = backgrounds
            .get(primary)
            .ok_or_else(|| format!("{primary} is not a connection background"))?;
        let palette = palette_of(primary);
        let mut sprites = BTreeMap::new();
        for (name, pic) in &backgrounds {
            sprites.insert(format!("{name}.PIC"), decode(pic, &palette_of(pic)));
        }
        for name in PIECES {
            sprites.insert(format!("{name}.PIC"), decode(&*find(name)?, &palette));
        }
        Ok(Self { sprites })
    }

    /// A kit from sprites already made, for tests and previews.
    pub fn from_sprites(sprites: BTreeMap<String, Sprite>) -> Self {
        Self { sprites }
    }

    /// One piece by retail name, with or without `.PIC`.
    pub fn sprite(&self, name: &str) -> &Sprite {
        let key = if name.ends_with(".PIC") {
            name.to_owned()
        } else {
            format!("{name}.PIC")
        };
        self.sprites
            .get(&key)
            .unwrap_or_else(|| panic!("the kit has no piece {key}"))
    }

    /// Every sprite, keyed `NAME.PIC`, for the menu's own button drawing.
    pub fn sprites(&self) -> &BTreeMap<String, Sprite> {
        &self.sprites
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one pixel picture of palette index 1, with its own 6 bit palette.
    fn pic(entry: [u8; 3]) -> Pic {
        let mut palette = vec![[0u8; 3]; 2];
        palette[1] = entry;
        Pic {
            width: 1,
            height: 1,
            pixels: vec![1],
            mask: vec![true],
            palette,
            glyphs: Vec::new(),
        }
    }
    fn pic_without_palette() -> Pic {
        Pic {
            palette: Vec::new(),
            ..pic([0; 3])
        }
    }
    /// Raw `Pic` bytes for the multiplayer map: kind 0, one pixel, no
    /// palette, no glyphs.
    fn raw_pic(index: u8) -> Vec<u8> {
        let mut data = vec![0u8; 65];
        data[2..6].copy_from_slice(&1u32.to_le_bytes());
        data[6..10].copy_from_slice(&1u32.to_le_bytes());
        data[10..14].copy_from_slice(&64u32.to_le_bytes());
        data[14..18].copy_from_slice(&1u32.to_le_bytes());
        data[18..22].copy_from_slice(&65u32.to_le_bytes());
        data[64] = index;
        data
    }

    #[test]
    fn pieces_come_from_both_maps_in_the_primary_palette() {
        let menu: BTreeMap<String, Pic> = ["ROCKER00", "PANELFNT", "ACTION0L"]
            .iter()
            .map(|n| (format!("{n}.PIC"), pic_without_palette()))
            .collect();
        // Every other piece is a multiplayer resource, index 1.
        let mut multiplayer: BTreeMap<String, Vec<u8>> = PIECES
            .iter()
            .filter(|n| !menu.contains_key(&format!("{n}.PIC")))
            .map(|n| (format!("{n}.PIC"), raw_pic(1)))
            .collect();
        // The backgrounds are decoded in their own palettes: give them
        // different ones.
        for (name, last) in [("MODEM3", 60u8), ("NETIPX3", 30u8)] {
            let mut bytes = raw_pic(1);
            // A two entry 6 bit palette after the pixel: black, `last` grey.
            bytes.extend_from_slice(&[0, 0, 0, last, last, last]);
            bytes[22..26].copy_from_slice(&6u32.to_le_bytes());
            multiplayer.insert(format!("{name}.PIC"), bytes);
        }
        let kit = Kit::new(&menu, &multiplayer, "MODEM3").unwrap();
        let modem = (60u16 * 255 + 31) / 63;
        let netipx = (30u16 * 255 + 31) / 63;
        assert_eq!(kit.sprite("MODEM3").rgba[..3], [modem as u8; 3]);
        assert_eq!(kit.sprite("NETIPX3").rgba[..3], [netipx as u8; 3]);
        // A piece with no palette of its own takes the primary picture's.
        assert_eq!(kit.sprite("ROCKER00").rgba[..3], [modem as u8; 3]);
        assert_eq!(kit.sprite("PANEL.PIC").rgba[..3], [modem as u8; 3]);
        // Another primary changes the pieces and leaves the backgrounds.
        let kit = Kit::new(&menu, &multiplayer, "NETIPX3").unwrap();
        assert_eq!(kit.sprite("PANEL").rgba[..3], [netipx as u8; 3]);
        assert_eq!(kit.sprite("MODEM3").rgba[..3], [modem as u8; 3]);
        // The sprite map has the names the menu's button drawing looks up.
        assert!(kit.sprites().contains_key("ACTION0L.PIC"));
    }

    #[test]
    fn a_missing_piece_asks_for_a_reimport() {
        let _ = pic;
        let error = Kit::new(&BTreeMap::new(), &BTreeMap::new(), "MODEM3")
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("MODEM3.PIC") && error.contains("re-import"),
            "{error}"
        );
        let mut multiplayer = BTreeMap::new();
        multiplayer.insert("MODEM3.PIC".to_string(), raw_pic(1));
        multiplayer.insert("NETIPX3.PIC".to_string(), raw_pic(1));
        let error = Kit::new(&BTreeMap::new(), &multiplayer, "MODEM3")
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("re-import"), "{error}");
        let error = Kit::new(&BTreeMap::new(), &multiplayer, "NOPE")
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("NOPE"), "{error}");
    }

    #[test]
    fn every_piece_is_in_an_import_list() {
        use tore_import::selection::{MENU_ART, MULTIPLAYER_ART};
        for name in PIECES.iter().chain(BACKGROUNDS.iter()) {
            let file = format!("{name}.PIC");
            assert!(
                MENU_ART.contains(&file.as_str()) || MULTIPLAYER_ART.contains(&file.as_str()),
                "{file} is in neither art list"
            );
        }
    }
}
