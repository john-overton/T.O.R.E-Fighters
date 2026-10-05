//! A kit of synthetic pieces for the tests: the retail names and sizes, flat
//! colours, fonts of fixed width. No retail bytes.
use super::{Kit, kit::BACKGROUNDS, kit::PIECES};
use crate::menu::Sprite;
use std::collections::BTreeMap;

fn flat(width: usize, height: usize, rgb: [u8; 3]) -> Sprite {
    Sprite {
        width,
        height,
        rgba: (0..width * height)
            .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
            .collect(),
        glyphs: Vec::new(),
    }
}

/// A font strip of 256 cells, each `cell` wide, every pixel lit.
pub fn font(cell: usize, height: usize) -> Sprite {
    font_in(cell, height, 255)
}

/// The same in a grey.
pub fn font_in(cell: usize, height: usize, grey: u8) -> Sprite {
    let mut sprite = flat(cell * 256, height, [grey; 3]);
    sprite.glyphs = (0..256).map(|i| [i * cell, cell, height]).collect();
    sprite
}

fn size(name: &str) -> (usize, usize) {
    match name {
        "PANEL" | "MODEM3" | "NETIPX3" => (640, 480),
        "EDGETL" | "EDGEBR" => (31, 35),
        "EDGETR" | "EDGEBL" => (30, 35),
        "EDGETB" => (47, 4),
        "EDGELR" => (4, 26),
        "ACTION0L" | "ACTIOD0L" => (24, 30),
        "ACTION0M" | "ACTIOD0M" => (8, 30),
        "ACTION0R" | "ACTIOD0R" => (29, 30),
        // The default buttons' pieces carry three more rows on top: the
        // outline, and the cap is 27 high.
        "ACTDFT0L" | "ACTDFD0L" => (24, 33),
        "ACTDFT0M" | "ACTDFD0M" => (8, 33),
        "ACTDFT0R" | "ACTDFD0R" => (29, 33),
        "ACTDFLT" | "ACTDFLD" => (20, 27),
        "LISTLFT" | "LISTRT" => (30, 17),
        "LISTMID" => (20, 17),
        "LISTHI" => (24, 12),
        "PAGEBOX" => (50, 17),
        n if n.starts_with("CHECK") => (28, 28),
        n if n.starts_with("ROCKER") => (27, 40),
        _ => (8, 8),
    }
}

/// Every piece the kit names, synthetic.
pub fn kit() -> Kit {
    let mut sprites = BTreeMap::new();
    for name in PIECES.iter().chain(BACKGROUNDS.iter()) {
        let sprite = match *name {
            "PANELFNT" => font(5, 10),
            "PANELFND" => font_in(5, 10, 150),
            "SMLFONT" => font(6, 12),
            "FONTACT" | "FONTACD" | "FONTDFT" | "FONTDFD" => font(7, 12),
            other => {
                let (w, h) = size(other);
                // Distinct colours so a drawn piece can be told from the
                // others in a test.
                let tone = other
                    .bytes()
                    .fold(7u8, |a, b| a.wrapping_mul(31).wrapping_add(b));
                flat(w, h, [tone, tone.wrapping_add(80), tone.wrapping_add(160)])
            }
        };
        sprites.insert(format!("{name}.PIC"), sprite);
    }
    Kit::from_sprites(sprites)
}

/// A blank canvas buffer.
pub fn blank() -> Vec<u8> {
    vec![0; crate::menu::WIDTH * crate::menu::HEIGHT * 4]
}

/// The colour at a canvas pixel.
pub fn at(pixels: &[u8], x: i32, y: i32) -> [u8; 3] {
    let i = (y as usize * crate::menu::WIDTH + x as usize) * 4;
    [pixels[i], pixels[i + 1], pixels[i + 2]]
}

/// The colour a piece of the synthetic kit is made of.
pub fn tone_of(kit: &Kit, name: &str) -> [u8; 3] {
    let sprite = kit.sprite(name);
    [sprite.rgba[0], sprite.rgba[1], sprite.rgba[2]]
}

/// The kit with one piece swapped.
pub fn with(mut kit: Kit, name: &str, sprite: Sprite) -> Kit {
    kit.sprites.insert(format!("{name}.PIC"), sprite);
    kit
}

/// A time to hang `Instant` arithmetic on.
pub fn start() -> std::time::Instant {
    std::time::Instant::now()
}
