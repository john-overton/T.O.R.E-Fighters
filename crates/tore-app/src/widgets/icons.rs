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
    /// The house: the player whose machine runs the game (EF8).
    House,
    /// The player's own mark: their slot in the lobby's Slots (EF8).
    You,
    /// A player whose game cannot play the mission (EF8).
    Unable,
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

const HOUSE: &[&str] = &[
    "....h....",
    "...hhh...",
    "..hhhhh..",
    ".hhhhhhh.",
    "..wwwww..",
    "..wwkww..",
    "..wwkww..",
    "..wwwww..",
];
const YOU: &[&str] = &[
    "Y......", "YYY....", "YYYYY..", "YYYYYYY", "YYYYY..", "YYY....", "Y......",
];
const UNABLE: &[&str] = &[
    "R.....R", ".R...R.", "..R.R..", "...R...", "..R.R..", ".R...R.", "R.....R",
];

fn colour(code: char) -> Option<[u8; 4]> {
    Some(match code {
        'g' => [255, 205, 60, 255],
        'd' => [176, 118, 24, 255],
        's' => [214, 218, 228, 255],
        'k' => [52, 44, 30, 255],
        'G' => [110, 235, 120, 255],
        'h' => [214, 112, 72, 255],
        'w' => [228, 216, 182, 255],
        'Y' => [140, 200, 255, 255],
        'R' => [255, 100, 88, 255],
        _ => return None,
    })
}

impl Icon {
    fn rows(self) -> &'static [&'static str] {
        match self {
            Icon::Lock => LOCK,
            Icon::Crown => CROWN,
            Icon::Ready => READY,
            Icon::House => HOUSE,
            Icon::You => YOU,
            Icon::Unable => UNABLE,
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
