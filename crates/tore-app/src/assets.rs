use crate::AppResult;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tore_formats::{Archive, Button, Pic};

const ART: &[&str] = &[
    "CHOOSEV.PIC",
    "ACTION0L.PIC",
    "ACTION0M.PIC",
    "ACTION0R.PIC",
    "ACTIOD0L.PIC",
    "ACTIOD0M.PIC",
    "ACTIOD0R.PIC",
    "FONTACT.PIC",
    "FONTACD.PIC",
    "MENUFONT.PIC",
    "BODYFONT.PIC",
];
const DATA: &[&str] = &[
    "CHOOSEAC.DLG",
    "MAINMENU.MNU",
    "FMENUD.MNU",
    "&CLICK.11K",
    "&BUTTON.11K",
    "&TOGGLE1.5K",
];
pub struct Assets {
    pub pics: BTreeMap<String, Pic>,
    pub buttons: Vec<Button>,
    pub sounds: BTreeMap<String, Vec<u8>>,
    pub palette: [[u8; 3]; 256],
}
pub fn data_directory() -> AppResult<PathBuf> {
    if let Some(path) = std::env::var_os("TORE_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    #[cfg(target_os = "macos")]
    let root = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is unset")?)
        .join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let root = PathBuf::from(std::env::var_os("APPDATA").ok_or("APPDATA is unset")?);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let root = match std::env::var_os("XDG_DATA_HOME") {
        Some(path) => PathBuf::from(path),
        None => {
            PathBuf::from(std::env::var_os("HOME").ok_or("HOME is unset")?).join(".local/share")
        }
    };
    Ok(root.join("T.O.R.E-Fighters"))
}
fn archive(root: &Path, name: &str) -> AppResult<Archive> {
    let path = fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
        })
        .ok_or_else(|| format!("{}: missing {name}", root.display()))?;
    if fs::metadata(&path)?.len() > 128 * 1024 * 1024 {
        return Err("archive exceeds initial importer limit of 128 MiB".into());
    }
    Ok(Archive::parse(fs::read(path)?)?)
}
impl Assets {
    fn decode(resources: &BTreeMap<String, Vec<u8>>) -> AppResult<Self> {
        let mut pics = BTreeMap::new();
        for name in ART {
            let bytes = resources
                .get(*name)
                .ok_or_else(|| format!("menu cache missing {name}; re-import media"))?;
            pics.insert(name.to_string(), Pic::parse(bytes)?);
        }
        let background = &pics["CHOOSEV.PIC"];
        if background.width != 640 || background.height != 480 || background.palette.len() != 256 {
            return Err("expected a 640x480 menu background with a full palette".into());
        }
        let palette = background
            .palette
            .clone()
            .try_into()
            .map_err(|_| "invalid background palette")?;
        for name in ["FONTACT.PIC", "FONTACD.PIC", "MENUFONT.PIC", "BODYFONT.PIC"] {
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
        Ok(Self {
            pics,
            buttons,
            sounds,
            palette,
        })
    }
    pub fn import(source: &Path, destination: &Path) -> AppResult<Self> {
        let mut resources = BTreeMap::new();
        let mut report = String::from(
            "T.O.R.E-Fighters menu import v1\nOnly selected resources decompressed. No executable resources executed.\n",
        );
        for (filename, names) in [("FA_1.LIB", ART), ("FA_2.LIB", DATA)] {
            let lib = archive(source, filename)?;
            report.push_str(&format!(
                "{filename}: {} unique entries\n",
                lib.entries.len()
            ));
            for name in names {
                let bytes = lib.read(name)?;
                let entry = &lib.entries[*name];
                report.push_str(&format!(
                    "{filename}/{name}: offset={}, stored={}, decoded={}\n",
                    entry.offset,
                    entry.size,
                    bytes.len()
                ));
                resources.insert(name.to_string(), bytes);
            }
        }
        // AIR003 is present as both XMI and recorded PCM. Its use here is a preview choice.
        match archive(source, "FA_4B.LIB").and_then(|lib| Ok(lib.read("AIR003.11K")?)) {
            Ok(bytes) => {
                report.push_str(&format!(
                    "FA_4B.LIB/AIR003.11K: {} decoded PCM bytes; menu preview mapping unverified\n",
                    bytes.len()
                ));
                resources.insert("AIR003.11K".into(), bytes);
            }
            Err(error) => {
                eprintln!("Music preview unavailable: {error}");
                report.push_str(&format!("Music preview unavailable: {error}\n"));
            }
        }
        let assets = Self::decode(&resources)?;
        fs::create_dir_all(destination)?;
        // Generation files keep the previous import usable until the new pack is complete.
        let generation = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let path = destination.join(format!("menu-{generation}.pack"));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(b"TOREMENU\x01\0\0\0")?;
        file.write_all(&(resources.len() as u32).to_le_bytes())?;
        for (name, bytes) in resources {
            file.write_all(&(name.len() as u16).to_le_bytes())?;
            file.write_all(name.as_bytes())?;
            file.write_all(&(bytes.len() as u32).to_le_bytes())?;
            file.write_all(&bytes)?;
        }
        file.sync_all()?;
        fs::write(destination.join("import-report.txt"), report)?;
        println!("Imported main-menu resources to {}", path.display());
        Ok(assets)
    }
    pub fn load(directory: &Path) -> AppResult<Self> {
        let mut paths = fs::read_dir(directory)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("menu-"))
                    && p.extension().is_some_and(|e| e == "pack")
            })
            .collect::<Vec<_>>();
        paths.sort();
        let mut last_error =
            "No imported menu. Run with --import gameassets/fighters-anthology".to_string();
        for path in paths.into_iter().rev() {
            match Self::load_pack(&path) {
                Ok(assets) => return Ok(assets),
                Err(error) => {
                    last_error = format!("{}: {error}", path.display());
                    eprintln!("Ignoring invalid menu cache: {last_error}");
                }
            }
        }
        Err(last_error.into())
    }
    fn load_pack(path: &Path) -> AppResult<Self> {
        let file = fs::File::open(path)?;
        if file.metadata()?.len() > 8 * 1024 * 1024 {
            return Err("menu pack exceeds 8 MiB".into());
        }
        let mut data = Vec::new();
        file.take(8 * 1024 * 1024 + 1).read_to_end(&mut data)?;
        let mut cursor = std::io::Cursor::new(data);
        let mut header = [0; 12];
        cursor.read_exact(&mut header)?;
        if &header != b"TOREMENU\x01\0\0\0" {
            return Err("unsupported menu pack".into());
        }
        fn word(c: &mut impl Read) -> AppResult<usize> {
            let mut b = [0; 4];
            c.read_exact(&mut b)?;
            Ok(u32::from_le_bytes(b) as usize)
        }
        let count = word(&mut cursor)?;
        if count > 64 {
            return Err("too many menu resources".into());
        }
        let mut resources = BTreeMap::new();
        for _ in 0..count {
            let mut len = [0; 2];
            cursor.read_exact(&mut len)?;
            let len = u16::from_le_bytes(len) as usize;
            if len == 0 || len > 32 {
                return Err("invalid resource name length".into());
            }
            let mut name = vec![0; len];
            cursor.read_exact(&mut name)?;
            let name = String::from_utf8(name)?;
            let length = word(&mut cursor)?;
            if length > 2 * 1024 * 1024 {
                return Err("menu resource exceeds limit".into());
            }
            let mut bytes = vec![0; length];
            cursor.read_exact(&mut bytes)?;
            if resources.insert(name, bytes).is_some() {
                return Err("duplicate menu resource".into());
            }
        }
        if cursor.position() != cursor.get_ref().len() as u64 {
            return Err("trailing menu pack bytes".into());
        }
        Self::decode(&resources)
    }
}
