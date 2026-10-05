//! The title bar lettering of the connection screens: the words the game
//! ships, and the picture a player can supply in their place.
//!
//! The screens show retail's `NETIPX3` (the NETWORK CONNECTION picture, from
//! the player's own import). The game covers the retail lettering with the
//! bar's own texture and draws DIRECT NETWORK CONNECTION over it
//! ([`built_in`], `assets/direct-network-connection-title.png`: the words in
//! Liberation Sans over a dark offset copy as a shadow, on a transparent
//! background, no retail pixels; `tools/build_title_lettering.py` makes it). A player who wants other words saves a PNG named [`FILE`] in the
//! game's data folder, which is used instead: it is at most 640 pixels wide and
//! its top [`ROWS`] rows are read, drawn with its transparency honoured and
//! fixed to the bar's top right, so a picture that is clear everywhere but the
//! wording touches only the wording. A full 640 by 480 picture works too.
//! The photograph below the bar stays the imported one.
//!
//! The player's file is their own, read at run time and never part of the
//! repository or a package: a bar made from retail's carries retail's badge
//! and stone texture (John, 2026-10-05, *opinionated*, requested).
//!
//! The Internet Lobby screen (slice I4) has its own words, INTERNET LOBBY
//! ([`Title::Internet`], `assets/internet-lobby-title.png`, lettered the same
//! way by `tools/build_title_lettering.py`), and its own player file,
//! `InternetLobby.png`, which is read the same way.
use super::png_read::decode_rows;
use crate::menu::{Sprite, WIDTH};
use std::path::Path;
use std::sync::OnceLock;

/// The lettering the game ships: 564 by 36, to be fixed to the top right.
const TITLE: &[u8] = include_bytes!("../../assets/direct-network-connection-title.png");
/// The Internet Lobby's lettering, made the same way.
const INTERNET_TITLE: &[u8] = include_bytes!("../../assets/internet-lobby-title.png");

/// Which screen's words the title bar carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Title {
    /// DIRECT NETWORK CONNECTION: the Direct Connection and lobby screens.
    Direct,
    /// INTERNET LOBBY: the Internet Lobby screen.
    Internet,
}

impl Title {
    /// The player's own file for this title, looked for in the data folder.
    pub const fn file(self) -> &'static str {
        match self {
            Self::Direct => FILE,
            Self::Internet => INTERNET_FILE,
        }
    }

    /// The lettering the game ships for this title.
    pub fn built_in(self) -> &'static Sprite {
        match self {
            Self::Direct => built_in(),
            Self::Internet => {
                static SPRITE: OnceLock<Sprite> = OnceLock::new();
                SPRITE.get_or_init(|| {
                    sprite(
                        decode_rows(INTERNET_TITLE, WIDTH, ROWS)
                            .expect("assets/internet-lobby-title.png is a PNG that fits the bar"),
                    )
                })
            }
        }
    }
}

fn sprite(image: super::png_read::Image) -> Sprite {
    Sprite {
        width: image.width,
        height: image.rows,
        rgba: image.rgba,
        glyphs: Vec::new(),
    }
}

/// The shipped lettering. Our own file, so a bad one is a build error and
/// panics.
pub fn built_in() -> &'static Sprite {
    static TITLE_SPRITE: OnceLock<Sprite> = OnceLock::new();
    TITLE_SPRITE.get_or_init(|| {
        sprite(
            decode_rows(TITLE, WIDTH, ROWS)
                .expect("assets/direct-network-connection-title.png is a PNG that fits the bar"),
        )
    })
}

/// The file looked for in the data folder for the Direct Connection title.
pub const FILE: &str = "DirectNetworkConnection.png";
/// The file looked for in the data folder for the Internet Lobby title.
pub const INTERNET_FILE: &str = "InternetLobby.png";
/// The rows of the picture that are used: `NETIPX3`'s title bar.
pub const ROWS: usize = super::panel::TITLE_BAR_ROWS as usize;
/// The largest file read, in bytes.
const LARGEST: u64 = 16 << 20;

/// The Direct Connection title bar in `dir`, or none when there is no such
/// file or it cannot be used (the log says why, and the retail bar stays).
pub fn load(dir: &Path) -> Option<Sprite> {
    load_title(dir, Title::Direct)
}

/// The player's picture for `title` in `dir`, as [`load`].
pub fn load_title(dir: &Path, title: Title) -> Option<Sprite> {
    let path = dir.join(title.file());
    let size = std::fs::metadata(&path).ok()?.len();
    let sprite = if size > LARGEST {
        Err(format!("it is {size} bytes, over {LARGEST}"))
    } else {
        std::fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|file| decode_rows(&file, WIDTH, ROWS))
            .map(sprite)
    };
    match sprite {
        Ok(sprite) => {
            log::info!("Title bar picture: {}", path.display());
            Some(sprite)
        }
        Err(why) => {
            log::warn!(
                "Cannot use the title bar picture {}: {why}; showing the retail bar",
                path.display()
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tore-header-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn no_file_means_no_header_and_a_bad_one_too() {
        let dir = folder("none");
        assert!(load(&dir).is_none());
        std::fs::write(dir.join(FILE), b"not a picture").unwrap();
        assert!(load(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A PNG of `width` by `height`, every row filter 0, RGBA, red rising by
    /// row, half transparent.
    fn rgba_png(width: usize, height: usize) -> Vec<u8> {
        use crate::replay::png::{crc32, zlib_stored};
        let mut lines = Vec::new();
        for row in 0..height {
            lines.push(0);
            for _ in 0..width {
                lines.extend_from_slice(&[row as u8, 20, 30, 128]);
            }
        }
        let mut file = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
        let mut chunk = |kind: &[u8; 4], body: &[u8]| {
            file.extend_from_slice(&(body.len() as u32).to_be_bytes());
            let start = file.len();
            file.extend_from_slice(kind);
            file.extend_from_slice(body);
            let crc = crc32(&file[start..]);
            file.extend_from_slice(&crc.to_be_bytes());
        };
        let mut head = Vec::new();
        head.extend_from_slice(&(width as u32).to_be_bytes());
        head.extend_from_slice(&(height as u32).to_be_bytes());
        head.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(b"IHDR", &head);
        chunk(b"IDAT", &zlib_stored(&lines));
        chunk(b"IEND", &[]);
        file
    }

    #[test]
    fn a_full_picture_gives_its_top_rows_and_a_narrow_one_its_own_size() {
        let dir = folder("full");
        std::fs::write(dir.join(FILE), rgba_png(WIDTH, ROWS + 3)).unwrap();
        let header = load(&dir).expect("a header");
        assert_eq!((header.width, header.height), (WIDTH, ROWS));
        let last = (ROWS - 1) * WIDTH * 4;
        assert_eq!(header.rgba[last..last + 4], [(ROWS - 1) as u8, 20, 30, 128]);
        // Narrower and shorter than the bar: kept as it is, fixed to the top
        // right when drawn.
        std::fs::write(dir.join(FILE), rgba_png(300, 20)).unwrap();
        let header = load(&dir).expect("a header");
        assert_eq!((header.width, header.height), (300, 20));
        // Wider than the canvas is refused.
        std::fs::write(dir.join(FILE), rgba_png(WIDTH + 1, 10)).unwrap();
        assert!(load(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_shipped_lettering_is_only_lettering_fixed_to_the_top_right() {
        let title = built_in();
        assert_eq!((title.width, title.height), (564, 36));
        // Mostly transparent, with no colour hidden under the clear pixels.
        let clear: Vec<&[u8]> = title.rgba.chunks_exact(4).filter(|px| px[3] == 0).collect();
        assert!(clear.len() > title.width * title.height / 2);
        assert!(
            clear.iter().all(|px| px[..3] == [0; 3]),
            "colour under clear"
        );
        // The ink spans the words and nothing else: 4 to 445 across, 7 to 30
        // down. Fixed to the top right it starts at x 80, where the retail
        // lettering did.
        let ink = |x: usize, y: usize| title.rgba[(y * title.width + x) * 4 + 3] > 0;
        let columns: Vec<usize> = (0..title.width)
            .filter(|x| (0..title.height).any(|y| ink(*x, y)))
            .collect();
        let rows: Vec<usize> = (0..title.height)
            .filter(|y| (0..title.width).any(|x| ink(x, *y)))
            .collect();
        assert_eq!((columns[0], columns[columns.len() - 1]), (4, 445));
        assert_eq!((rows[0], rows[rows.len() - 1]), (7, 30));
        assert_eq!(WIDTH - title.width + columns[0], 80);
    }

    #[test]
    fn each_screen_has_its_own_file_and_its_own_shipped_words() {
        let dir = folder("internet");
        std::fs::write(dir.join(FILE), rgba_png(300, 20)).unwrap();
        // The Direct Connection file does not stand in for the Internet one.
        assert!(load_title(&dir, Title::Direct).is_some());
        assert!(load_title(&dir, Title::Internet).is_none());
        std::fs::write(dir.join(INTERNET_FILE), rgba_png(200, 10)).unwrap();
        let internet = load_title(&dir, Title::Internet).expect("a header");
        assert_eq!((internet.width, internet.height), (200, 10));
        assert_ne!(Title::Direct.file(), Title::Internet.file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_internet_lobby_lettering_is_lettering_fixed_to_the_top_right() {
        let title = Title::Internet.built_in();
        assert_eq!((title.width, title.height), (564, 36));
        let clear: Vec<&[u8]> = title.rgba.chunks_exact(4).filter(|px| px[3] == 0).collect();
        assert!(clear.len() > title.width * title.height * 3 / 4);
        assert!(
            clear.iter().all(|px| px[..3] == [0; 3]),
            "colour under clear"
        );
        let ink = |x: usize, y: usize| title.rgba[(y * title.width + x) * 4 + 3] > 0;
        let columns: Vec<usize> = (0..title.width)
            .filter(|x| (0..title.height).any(|y| ink(*x, y)))
            .collect();
        let rows: Vec<usize> = (0..title.height)
            .filter(|y| (0..title.width).any(|x| ink(x, *y)))
            .collect();
        // Starts where the other lettering does, x 80 on the canvas, and is
        // the same height; INTERNET LOBBY is shorter than the other words.
        assert_eq!(WIDTH - title.width + columns[0], 80);
        assert_eq!((rows[0], rows[rows.len() - 1]), (7, 30));
        assert!(columns[columns.len() - 1] < 445);
    }
}
