use crate::AppResult;
use std::{collections::BTreeMap, path::Path};
use tore_formats::{Button, Pic};
use tore_import::media_source::MediaSource;

pub use tore_import::{Progress, data_directory};

pub struct Assets {
    pub creator_options: tore_formats::ui::creator::Options,
    pub theater_resources: BTreeMap<String, Vec<u8>>,
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

impl Assets {
    fn decode(resources: &tore_import::Resources) -> AppResult<Self> {
        tore_import::check_markers(resources)?;
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
                .filter(|(name, _)| !tore_formats::music::resource(name))
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
