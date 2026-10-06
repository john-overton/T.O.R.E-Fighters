//! The small marks a list row can carry: the padlock, crown, ready tick, house,
//! the player's own arrow, the unable cross, the three platform marks, the relay
//! mark and the standby host's outlined house.
//! Retail has no art for them, so they are our own: minimalist solid shapes in
//! one colour, hand drawn as SVG on a 16 by 16 grid
//! (`crates/tore-app/assets/icons/`) and baked into the sharp text atlas by
//! `tools/build_ui_text_atlas.py`. Drawn for the window they are sharp
//! ([`crate::ui_text::icon`]); otherwise the atlas's picture is averaged down
//! to the icon's size and drawn into the canvas.
//!
//! John asked on 2026-10-05 for the crown and the house to be cleaned up, the
//! platform marks to be added, and every icon to be a hand drawn minimalist
//! SVG in one colour (*opinionated*). The first set was pixel pictures in six
//! colours, authored by an agent (EF2, EF8).
use crate::menu::{Canvas, HEIGHT, WIDTH};
use crate::ui_text;

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
    /// A player on Windows.
    Windows,
    /// A player on macOS.
    MacOs,
    /// A player on Linux.
    Linux,
    /// A player whose game reaches the host through the master's relay
    /// (slice J6).
    Relay,
    /// A player whose game stands by to take over hosting (stage K, slice
    /// K7b): the house, outlined, where the house itself has the solid one.
    Standby,
}

/// The one colour of every icon, before a dimmed row's tint.
pub const COLOUR: [u8; 3] = [228, 230, 236];

/// The side of an icon on the canvas, in pixels. Every icon is square.
const SIZE: i32 = 12;

impl Icon {
    /// The mark for a platform, none for one the protocol does not name.
    pub fn of_platform(platform: tore_session::wire::Platform) -> Option<Icon> {
        use tore_session::wire::Platform;
        match platform {
            Platform::Windows => Some(Icon::Windows),
            Platform::MacOs => Some(Icon::MacOs),
            Platform::Linux => Some(Icon::Linux),
            Platform::Unknown => None,
        }
    }
    /// The mark for how a player reached the host: only the relay has one,
    /// since a relayed player is the one the others may want to know about
    /// (its path is a little slower and it is never the calculated host).
    pub fn of_path(path: tore_session::wire::Path) -> Option<Icon> {
        use tore_session::wire::Path;
        match path {
            Path::Relay => Some(Icon::Relay),
            _ => None,
        }
    }
    /// The icon's name in the atlas, and its SVG's file name.
    pub fn name(self) -> &'static str {
        match self {
            Icon::Lock => "lock",
            Icon::Crown => "crown",
            Icon::Ready => "ready",
            Icon::House => "house",
            Icon::You => "you",
            Icon::Unable => "unable",
            Icon::Windows => "windows",
            Icon::MacOs => "macos",
            Icon::Linux => "linux",
            Icon::Relay => "relay",
            Icon::Standby => "standby",
        }
    }
    /// Width and height in pixels.
    pub fn size(self) -> (i32, i32) {
        (SIZE, SIZE)
    }
    /// Draws the icon with its top left at `(x, y)`. `tint` is a dimmed row's
    /// grey; the icon is otherwise [`COLOUR`] whatever the row's own colour.
    pub fn draw(self, canvas: &mut Canvas, (x, y): (i32, i32), tint: Option<[u8; 3]>) {
        let colour = match tint {
            Some(tint) => std::array::from_fn(|c| {
                ((u32::from(COLOUR[c]) * u32::from(tint[c]) + 127) / 255) as u8
            }),
            None => COLOUR,
        };
        if ui_text::icon(self.name(), (x, y, SIZE, SIZE), colour, None) {
            return;
        }
        let Some(mask) = ui_text::icon_mask(self.name(), SIZE as usize) else {
            return;
        };
        for (i, coverage) in mask.iter().enumerate() {
            let (px, py) = (x + i as i32 % SIZE, y + i as i32 / SIZE);
            if *coverage == 0 || px < 0 || py < 0 || px >= WIDTH as i32 || py >= HEIGHT as i32 {
                continue;
            }
            let at = (py as usize * WIDTH + px as usize) * 4;
            let alpha = u32::from(*coverage);
            for (c, ink) in colour.iter().enumerate() {
                canvas.0[at + c] =
                    ((u32::from(*ink) * alpha + u32::from(canvas.0[at + c]) * (255 - alpha) + 127)
                        / 255) as u8;
            }
            canvas.0[at + 3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_text;
    use crate::widgets::test_kit::blank;

    const ALL: [Icon; 11] = [
        Icon::Lock,
        Icon::Crown,
        Icon::Ready,
        Icon::House,
        Icon::You,
        Icon::Unable,
        Icon::Windows,
        Icon::MacOs,
        Icon::Linux,
        Icon::Relay,
        Icon::Standby,
    ];

    #[test]
    fn every_icon_is_in_the_atlas_and_the_same_square_size() {
        for icon in ALL {
            assert_eq!(icon.size(), (12, 12));
            let mask = ui_text::icon_mask(icon.name(), 12)
                .unwrap_or_else(|| panic!("the atlas has no {} icon", icon.name()));
            assert_eq!(mask.len(), 144);
            assert!(
                mask.iter().filter(|c| **c > 64).count() > 8,
                "{} has a body",
                icon.name()
            );
        }
        // Each svg in the assets folder is an icon here.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
        let mut files: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        let mut names: Vec<String> = ALL.iter().map(|i| format!("{}.svg", i.name())).collect();
        names.sort();
        assert_eq!(files, names);
    }

    #[test]
    fn each_platform_the_wire_names_has_its_own_mark() {
        use tore_session::wire::Platform;
        let marks: Vec<Option<Icon>> = Platform::ALL
            .iter()
            .map(|p| Icon::of_platform(*p))
            .collect();
        assert_eq!(
            marks,
            [
                None,
                Some(Icon::Windows),
                Some(Icon::MacOs),
                Some(Icon::Linux)
            ]
        );
    }

    #[test]
    fn only_a_relayed_path_has_a_mark() {
        use tore_session::wire::Path;
        let marks: Vec<Option<Icon>> = Path::ALL.iter().map(|p| Icon::of_path(*p)).collect();
        assert_eq!(
            marks,
            [None, None, None, None, None, Some(Icon::Relay)],
            "local network, by address, mapped port, IPv6, punched, relay"
        );
    }

    #[test]
    fn an_icon_is_drawn_in_one_colour_into_the_canvas_when_not_recording() {
        let mut pixels = blank();
        for icon in ALL {
            pixels.fill(0);
            icon.draw(&mut Canvas(&mut pixels), (100, 100), None);
            let lit: Vec<&[u8]> = pixels.chunks_exact(4).filter(|px| px[3] != 0).collect();
            assert!(lit.len() > 20, "{} drew {} pixels", icon.name(), lit.len());
            // Every pixel is the colour at some coverage over black: no other
            // hue, so a pixel is never brighter than the colour.
            for px in lit {
                for c in 0..3 {
                    assert!(px[c] <= COLOUR[c], "{}: {:?}", icon.name(), px);
                }
            }
        }
        // A dimmed row's tint darkens it.
        let mut bright = blank();
        let mut dim = blank();
        Icon::Crown.draw(&mut Canvas(&mut bright), (10, 10), None);
        Icon::Crown.draw(&mut Canvas(&mut dim), (10, 10), Some([118; 3]));
        let sum = |p: &[u8]| p.iter().map(|b| u64::from(*b)).sum::<u64>();
        assert!(sum(&dim) < sum(&bright));
    }

    #[test]
    fn a_recorded_icon_is_kept_for_the_sharp_layer_and_not_drawn() {
        let mut pixels = blank();
        ui_text::begin();
        Icon::House.draw(&mut Canvas(&mut pixels), (20, 30), None);
        let layer = ui_text::finish().unwrap();
        assert!(
            pixels.iter().all(|b| *b == 0),
            "nothing went into the canvas"
        );
        let quads = layer.quads(false);
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].dst, [20.0, 30.0, 12.0, 12.0]);
        // A panel drawn over it afterwards hides it.
        ui_text::begin();
        Icon::House.draw(&mut Canvas(&mut pixels), (20, 30), None);
        ui_text::occlude((0, 0, 640, 480));
        let layer = ui_text::finish().unwrap();
        assert!(layer.quads(false).is_empty());
    }
}
