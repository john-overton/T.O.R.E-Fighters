use crate::AppResult;
use std::{collections::BTreeMap, path::Path};
use tore_formats::{Button, Pic};
use tore_import::media_source::MediaSource;

pub use tore_import::{Progress, data_directory};

pub struct Assets {
    pub creator_options: tore_formats::ui::creator::Options,
    pub theater_resources: BTreeMap<String, Vec<u8>>,
    /// The multiplayer screens' pictures, dialogs, menus and the quick-message
    /// file, as imported (the screens decode them with their own palettes).
    /// Kept apart from `theater_resources` on purpose: that map is what the
    /// combat tapes fingerprint, so the same flights keep the same tapes when
    /// the import grows by menu art.
    #[allow(dead_code)] // Read by the multiplayer screens (EF2 onwards).
    pub multiplayer_resources: BTreeMap<String, Vec<u8>>,
    pub pics: BTreeMap<String, Pic>,
    pub buttons: Vec<Button>,
    pub sounds: BTreeMap<String, Vec<u8>>,
    pub music_scores: BTreeMap<String, Vec<u8>>,
    pub palette: [[u8; 3]; 256],
}
/// A finished import: the decoded assets and the plain-words summary the
/// locate screen shows. The same facts are in `import-report.txt` in full.
pub(crate) struct ImportOutcome {
    pub assets: Assets,
    /// Shown by the pre-game shell as the import summary.
    pub summary: Vec<String>,
}

/// Confirms the multiplayer screens' pieces (slice EF1) are in the pack and
/// that each picture, dialog and menu reads with the existing readers. The
/// screens decode the pictures themselves with their own palettes, so nothing
/// is kept here. A missing `CHAT.TXT` is not a failure: chat then has no quick
/// messages.
fn check_multiplayer_art(resources: &tore_import::Resources) -> AppResult<()> {
    use tore_import::selection::{MULTIPLAYER_ART, MULTIPLAYER_DATA};
    for name in MULTIPLAYER_ART.iter().chain(MULTIPLAYER_DATA) {
        let bytes = resources
            .get(*name)
            .ok_or_else(|| format!("cache missing multiplayer {name}; re-import media"))?;
        if name.ends_with(".PIC") {
            Pic::parse(bytes).map_err(|error| format!("multiplayer {name}: {error}"))?;
        } else if name.ends_with(".DLG") {
            tore_formats::ui::dialog::parse(bytes)
                .map_err(|error| format!("multiplayer {name}: {error}"))?;
        } else if name.ends_with(".MNU") {
            tore_formats::ui::menu_tree(bytes)
                .map_err(|error| format!("multiplayer {name}: {error}"))?;
        }
    }
    Ok(())
}

/// True for what the multiplayer slice (EF1) added to the pack: the pieces in
/// `MULTIPLAYER_ART` and `MULTIPLAYER_DATA`, the scroll bar's track
/// (`SLIDER_ART`, optional), the quick-message file and the
/// marker, and the import's source entry (slice L1: the build and importer,
/// which no flight depends on).
fn is_multiplayer_resource(name: &str) -> bool {
    use tore_import::selection::{CHAT_RESOURCE, MULTIPLAYER_ART, MULTIPLAYER_DATA, SLIDER_ART};
    MULTIPLAYER_ART.contains(&name)
        || SLIDER_ART.contains(&name)
        || MULTIPLAYER_DATA.contains(&name)
        || name == CHAT_RESOURCE
        || name == tore_import::pack::MULTIPLAYER_MARKER
        || name == tore_import::source::RESOURCE
}

impl Assets {
    fn decode(resources: &tore_import::Resources) -> AppResult<Self> {
        tore_import::check_markers(resources)?;
        // The dedicated server does not ask for this; the game does, because
        // the multiplayer screens draw art an older import did not keep.
        tore_import::check_multiplayer_marker(resources)?;
        for &name in tore_formats::ui::creator::ORDNANCE_SOUNDS {
            let bytes = resources
                .get(name)
                .ok_or_else(|| format!("cache missing ordnance sound {name}; re-import media"))?;
            tore_formats::pcm::Pcm::parse(name, bytes)?;
        }
        for &name in tore_formats::aircraft::COMBAT_RESOURCES {
            if !resources.contains_key(name) {
                return Err(
                    format!("cache missing combat resource {name}; re-import media").into(),
                );
            }
        }
        let mut music_scores = BTreeMap::new();
        for name in tore_formats::music::SCORES {
            if let Some(bytes) = resources.get(*name) {
                tore_formats::music::Score::parse(bytes)?;
                music_scores.insert(name.to_string(), bytes.clone());
            }
        }
        for name in [
            "&GEARUP.5K",
            "&STALLWR.5K",
            "&STALL.5K",
            "&RWRLOCK.5K",
            "&RWRDTCT.5K",
            "&RWRIR.5K",
            "WIN11.FNT",
            "HUDSYM11.FNT",
            "HUD11.FNT",
            "RAFALE.PT",
            "RAFALE.HUD",
            "~RAFH.PIC",
            "F18.PT",
            "F18.HUD",
            "~F18H.PIC",
            "WIN01.FNT",
            "UKR.T2",
            "UKR.MM",
            "TORE_TERRAIN_V2",
            "SUN.SH",
            "MOON.SH",
            "STARS.SH",
            "OCEAN0.PIC",
            "_MOON.PIC",
            "_CLOUD1.PIC",
        ] {
            if !resources.contains_key(name) {
                return Err(format!("cache missing {name}; re-import media").into());
            }
        }
        for (code, _) in tore_formats::theater::THEATERS {
            if !resources.contains_key(&format!("{code}.MM")) {
                return Err(format!("cache missing {code}.MM; re-import all theaters").into());
            }
        }
        for name in tore_import::selection::DEBRIEF_ART
            .iter()
            .chain(tore_import::selection::DEBRIEF_DATA)
        {
            if !resources.contains_key(*name) {
                return Err(format!("cache missing debrief {name}; re-import media").into());
            }
        }
        if !resources.contains_key("TVI0.PIC") {
            return Err("cache missing Vietnam textures; re-import media".into());
        }
        for id in tore_formats::aircraft::AircraftId::ALL {
            for name in [
                id.hud().to_string(),
                id.cockpit().to_string(),
                id.instrument_panel(),
                format!("{}.SH", id.stem()),
                format!("_{}.PIC", id.stem()),
            ] {
                if !resources.contains_key(&name) {
                    return Err(format!("cache missing {name}; re-import media").into());
                }
            }
            tore_formats::aircraft::Aircraft::parse(
                resources
                    .get(id.pt())
                    .ok_or_else(|| format!("cache missing {}; re-import media", id.pt()))?,
            )?;
        }
        tore_formats::font::Font::parse(&resources["WIN11.FNT"])?;
        let mut pics = BTreeMap::new();
        for name in tore_import::selection::MENU_ART {
            let bytes = resources
                .get(*name)
                .ok_or_else(|| format!("menu cache missing {name}; re-import media"))?;
            pics.insert(name.to_string(), Pic::parse(bytes)?);
        }
        check_multiplayer_art(resources)?;
        for (name, bytes) in resources.iter().filter(|(n, _)| {
            n.starts_with('$') && n.ends_with(".PIC")
                || ["MCICONS.PIC", "FNTWPNB.PIC", "FNTWPNY.PIC"].contains(&n.as_str())
        }) {
            pics.insert(name.clone(), Pic::parse(bytes)?);
        }
        for name in [
            "CHOOSEV.PIC",
            "CHOOSEAC.PIC",
            "CHOOSE3.PIC",
            "CHOOSEU.PIC",
            "CHOOSEM.PIC",
        ] {
            let background = &pics[name];
            if background.width != 640
                || background.height != 480
                || background.palette.len() != 256
            {
                return Err("expected 640x480 menu backgrounds with full palettes".into());
            }
        }
        let background = &pics["CHOOSEV.PIC"];
        let palette = background
            .palette
            .clone()
            .try_into()
            .map_err(|_| "invalid background palette")?;
        for name in [
            "FONTACT.PIC",
            "FONTACD.PIC",
            "MENUFONT.PIC",
            "BODYFONT.PIC",
            "ARMFONT.PIC",
            "SMLFONT.PIC",
        ] {
            if pics[name].glyphs.len() != 256 {
                return Err(format!("{name}: missing glyph table").into());
            }
        }
        for prefix in ["ACTION0", "ACTIOD0"] {
            for part in ["L", "M", "R"] {
                let pic = &pics[&format!("{prefix}{part}.PIC")];
                if pic.width == 0 || pic.width > 32 || pic.height != 30 {
                    return Err("unsupported action sprite dimensions".into());
                }
            }
        }
        let buttons = tore_formats::activity_buttons(
            resources
                .get("CHOOSEAC.DLG")
                .ok_or("missing CHOOSEAC.DLG")?,
        )?;
        let sounds = resources
            .iter()
            .filter(|(name, _)| name.ends_with(".11K") || name.ends_with(".5K"))
            .map(|(name, bytes)| (name.clone(), bytes.clone()))
            .collect::<BTreeMap<_, _>>();
        if sounds.values().any(|s| s.is_empty() || s.len() > 1_000_000) {
            return Err("invalid menu PCM size".into());
        }
        tore_formats::weather::clouds::Layout::decode(
            resources
                .get("TORE_CLOUDS_V1")
                .ok_or("cache predates cloud layout; re-import media")?,
        )?;
        tore_formats::weather::flare::Layout::decode(
            resources
                .get("TORE_FLARE_V1")
                .ok_or("cache predates lens flare; re-import media")?,
        )?;
        let creator_options = tore_formats::ui::creator::Options::decode(
            resources
                .get("TORE_CREATOR_V1")
                .ok_or("cache predates creator options; re-import media")?,
        )?;
        Ok(Self {
            creator_options,
            theater_resources: resources
                .iter()
                .filter(|(name, _)| {
                    !tore_formats::music::resource(name) && !is_multiplayer_resource(name)
                })
                .map(|(n, b)| (n.clone(), b.clone()))
                .collect(),
            multiplayer_resources: resources
                .iter()
                .filter(|(name, _)| is_multiplayer_resource(name))
                .map(|(n, b)| (n.clone(), b.clone()))
                .collect(),
            pics,
            buttons,
            sounds,
            music_scores,
            palette,
        })
    }
    /// Import from a path the caller has not classified yet, for the CLI.
    pub(crate) fn import_path(source: &Path, destination: &Path) -> AppResult<Self> {
        // The plain-words reason is what a terminal user needs, not the variant.
        let source = MediaSource::detect(source).map_err(|error| error.to_string())?;
        Self::import(&source, destination)
    }
    pub(crate) fn import(source: &MediaSource, destination: &Path) -> AppResult<Self> {
        Ok(Self::import_with_progress(source, destination, &mut |_| {})?.assets)
    }
    /// Import with progress reports, at least once per archive and every 64
    /// resources. The callback runs on the importing thread. The library
    /// writes and verifies the pack; this only decodes what the menus draw.
    pub(crate) fn import_with_progress(
        source: &MediaSource,
        destination: &Path,
        progress: &mut dyn FnMut(Progress),
    ) -> AppResult<ImportOutcome> {
        let imported =
            tore_import::import_with_progress(source, destination, progress, &Self::decode)?;
        Ok(ImportOutcome {
            assets: imported.value,
            summary: imported.summary,
        })
    }
    pub fn load(directory: &Path) -> AppResult<Self> {
        Ok(tore_import::load_with(directory, &Self::decode)?.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_import::{Resources, selection};

    const OLD_MARKERS: [(&str, &[u8]); 5] = [
        ("TORE_MUSIC_V1", b"PCM1"),
        ("TORE_COMBAT_V1", b"RAW1"),
        ("TORE_AIRPORTS_V1", b"SCENE1"),
        ("TORE_SPEECH_V1", b"ALL1"),
        ("TORE_SURFACE_V1", b"SURF1"),
    ];

    fn older_pack() -> Resources {
        OLD_MARKERS
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_vec()))
            .collect()
    }

    #[test]
    fn the_game_asks_for_a_re_import_of_a_pack_without_the_multiplayer_art() {
        let resources = older_pack();
        // The dedicated server's check takes this pack; the game's refuses it.
        tore_import::check_markers(&resources).unwrap();
        let error = Assets::decode(&resources).err().unwrap().to_string();
        assert!(error.contains("multiplayer"), "{error}");
        assert!(error.contains("re-import media"), "{error}");
    }

    #[test]
    fn the_multiplayer_pieces_stay_out_of_the_tape_fingerprinted_resources() {
        for name in selection::MULTIPLAYER_ART
            .iter()
            .chain(selection::MULTIPLAYER_DATA)
        {
            assert!(is_multiplayer_resource(name), "{name}");
        }
        // The scroll bar's track is one, and no pack is refused for lacking it.
        for name in selection::SLIDER_ART {
            assert!(is_multiplayer_resource(name), "{name}");
            assert!(!selection::MULTIPLAYER_ART.contains(name), "{name}");
        }
        assert!(is_multiplayer_resource(selection::CHAT_RESOURCE));
        assert!(is_multiplayer_resource("TORE_MULTIPLAYER_V1"));
        assert!(is_multiplayer_resource(tore_import::source::RESOURCE));
        // What flights already depended on is not touched.
        for name in [
            "PANELFNT.PIC",
            "TORE_SPEECH_V1",
            "F18.PT",
            "CHOOSEM.PIC",
            "FMENUD.MNU",
        ] {
            assert!(!is_multiplayer_resource(name), "{name}");
        }
    }

    #[test]
    fn a_missing_or_unreadable_piece_is_named() {
        let mut resources = Resources::new();
        let error = check_multiplayer_art(&resources).unwrap_err().to_string();
        assert!(
            error.contains(selection::MULTIPLAYER_ART[0]) && error.contains("re-import media"),
            "{error}"
        );
        for name in selection::MULTIPLAYER_ART
            .iter()
            .chain(selection::MULTIPLAYER_DATA)
        {
            resources.insert(name.to_string(), vec![0; 16]);
        }
        let error = check_multiplayer_art(&resources).unwrap_err().to_string();
        assert!(error.contains(selection::MULTIPLAYER_ART[0]), "{error}");
    }

    /// Every piece the lists name, straight from the retail archives, when the
    /// install is present (the `gameassets` link or `TORE_GAME_DIR`); the test
    /// skips quietly otherwise. Also reads `CHAT.TXT` as the import does.
    #[test]
    fn the_retail_pieces_all_pass_the_pack_check() {
        let root = std::env::var_os("TORE_GAME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology")
            });
        let (Ok(art), Ok(data)) = (
            tore_formats::Archive::open(root.join("FA_1.LIB")),
            tore_formats::Archive::open(root.join("FA_2.LIB")),
        ) else {
            eprintln!("skipped: no retail install");
            return;
        };
        let mut resources = Resources::new();
        for (archive, names) in [
            (&art, selection::MULTIPLAYER_ART),
            (&data, selection::MULTIPLAYER_DATA),
        ] {
            for name in names {
                resources.insert(name.to_string(), archive.read(name).unwrap());
            }
        }
        check_multiplayer_art(&resources).unwrap();
        // The scroll bar's track is in the archive too (optional in a pack).
        for name in selection::SLIDER_ART {
            Pic::parse(&art.read(name).unwrap()).unwrap();
        }
    }
}
