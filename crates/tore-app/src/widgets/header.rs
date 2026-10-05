//! The title bar picture a player can supply for the connection screens.
//!
//! The screens show retail's `NETIPX3` (the NETWORK CONNECTION picture, from
//! the player's own import). A player who wants the bar to say something
//! else, such as DIRECT NETWORK CONNECTION, saves a PNG named [`FILE`] in the
//! game's data folder: its top [`ROWS`] rows are drawn over the bar, with its
//! transparency honoured, so a picture that is clear everywhere but the
//! wording touches only the wording. A full 640 by 480 picture works, only
//! its top rows are read. The photograph below the bar stays the imported
//! one.
//!
//! The file is the player's own, read at run time and never part of the
//! repository or a package: a bar made from retail's carries retail's badge
//! and stone texture (John, 2026-10-05, *opinionated*, requested).
use super::png_read::decode_top;
use crate::menu::{Sprite, WIDTH};
use std::path::Path;

/// The file looked for in the data folder.
pub const FILE: &str = "DirectNetworkConnection.png";
/// The rows of the picture that are used: `NETIPX3`'s title bar.
pub const ROWS: usize = super::panel::TITLE_BAR_ROWS as usize;
/// The largest file read, in bytes.
const LARGEST: u64 = 16 << 20;

/// The title bar in `dir`, or none when there is no such file or it cannot
/// be used (the log says why, and the retail bar stays).
pub fn load(dir: &Path) -> Option<Sprite> {
    let path = dir.join(FILE);
    let size = std::fs::metadata(&path).ok()?.len();
    let sprite = if size > LARGEST {
        Err(format!("it is {size} bytes, over {LARGEST}"))
    } else {
        std::fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|file| decode_top(&file, WIDTH, ROWS))
            .map(|image| Sprite {
                width: image.width,
                height: image.rows,
                rgba: image.rgba,
                glyphs: Vec::new(),
            })
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

    #[test]
    fn a_picture_of_the_right_width_gives_its_top_rows() {
        use crate::replay::png::{crc32, zlib_stored};
        let (width, height) = (WIDTH, ROWS + 3);
        // Every row filter 0, RGBA, red rising by row, half transparent.
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
        let dir = folder("full");
        std::fs::write(dir.join(FILE), &file).unwrap();
        let header = load(&dir).expect("a header");
        assert_eq!((header.width, header.height), (WIDTH, ROWS));
        let last = (ROWS - 1) * WIDTH * 4;
        assert_eq!(header.rgba[last..last + 4], [(ROWS - 1) as u8, 20, 30, 128]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
