//! The small marks a list row can carry. Retail has no art for them (the
//! survey found no padlock, crown or tick among the connection pieces), so
//! they are authored pixel pictures drawn from the tables below, not retail
//! bytes (*agent decision*, EF2).
use crate::menu::{Canvas, HEIGHT, WIDTH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// A game with a password.
    Lock,
    /// The King, the player who built the mission.
    Crown,
    /// A player who is ready to fly.
    Ready,
}

const LOCK: &[&str] = &[
    "..sssss..",
    ".ss...ss.",
    ".s.....s.",
    ".s.....s.",
    "ddddddddd",
    "dgggggggd",
    "dgggkgggd",
    "dggkkkggd",
    "dgggkgggd",
    "dgggggggd",
    "ddddddddd",
];
const CROWN: &[&str] = &[
    "g....g....g",
    "gg...g...gg",
    "ggg.ggg.ggg",
    "ggggggggggg",
    "ggggggggggg",
    "ddddddddddd",
];
const READY: &[&str] = &[
    ".........G",
    "........GG",
    ".......GG.",
    "G.....GG..",
    "GG...GG...",
    ".GG.GG....",
    "..GGG.....",
    "...G......",
];

fn colour(code: char) -> Option<[u8; 4]> {
    Some(match code {
        'g' => [255, 205, 60, 255],
        'd' => [176, 118, 24, 255],
        's' => [214, 218, 228, 255],
        'k' => [52, 44, 30, 255],
        'G' => [110, 235, 120, 255],
        _ => return None,
    })
}

impl Icon {
    fn rows(self) -> &'static [&'static str] {
        match self {
            Icon::Lock => LOCK,
            Icon::Crown => CROWN,
            Icon::Ready => READY,
        }
    }
    /// Width and height in pixels.
    pub fn size(self) -> (i32, i32) {
        let rows = self.rows();
        (rows[0].len() as i32, rows.len() as i32)
    }
    pub fn draw(self, canvas: &mut Canvas, (x, y): (i32, i32)) {
        for (row, line) in self.rows().iter().enumerate() {
            for (col, code) in line.chars().enumerate() {
                let (px, py) = (x + col as i32, y + row as i32);
                if let Some(rgba) = colour(code)
                    && px >= 0
                    && py >= 0
                    && px < WIDTH as i32
                    && py < HEIGHT as i32
                {
                    canvas.rect((px, py, 1, 1), rgba);
                }
            }
        }
    }
}
