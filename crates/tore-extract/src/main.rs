//! Generic EALIB extraction. No graphics, audio, Python, or retail fixtures required.
use std::{
    collections::HashSet,
    error::Error,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use tore_formats::{Archive, esa};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// One archive to extract: a loose file, or an entry stored inside a `SETUP.ESA`.
struct ArchiveSource {
    path: PathBuf,
    entry: Option<String>,
}
impl ArchiveSource {
    fn open(&self) -> Result<Archive> {
        Ok(match &self.entry {
            None => Archive::open(&self.path)?,
            Some(name) => esa::Container::open(&self.path)?.archive(name)?,
        })
    }
    /// Source-relative label with container provenance, e.g. `SETUP.ESA:FA_1.LIB`.
    fn label(&self, root: &Path) -> Result<String> {
        let mut text = self
            .path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        if let Some(name) = &self.entry {
            text.push(':');
            text.push_str(name);
        }
        Ok(text)
    }
    /// Output location: one directory level per archive boundary, container included.
    fn relative(&self, root: &Path) -> Result<PathBuf> {
        let mut path = self.path.strip_prefix(root)?.to_path_buf();
        if let Some(name) = &self.entry {
            path.push(name);
        }
        Ok(path)
    }
    fn display(&self) -> String {
        match &self.entry {
            None => self.path.to_string_lossy().into_owned(),
            Some(name) => format!("{}:{name}", self.path.to_string_lossy()),
        }
    }
}
struct Options {
    source: PathBuf,
    out: PathBuf,
    patterns: Vec<String>,
    exclude_archives: Vec<String>,
    theater: Option<String>,
    aircraft: Vec<tore_formats::aircraft::AircraftId>,
    weapons: bool,
    music: bool,
    creator: bool,
    wav_previews: bool,
    list: bool,
    dry_run: bool,
    overwrite: bool,
    limit: usize,
}
struct Record {
    archive: String,
    name: String,
    output: String,
    offset: usize,
    stored: usize,
    decoded: usize,
    status: &'static str,
    error: String,
    analysis: String,
    preview: Option<String>,
}
fn quote(value: &str) -> String {
    let mut s = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if c < ' ' => s.push_str(&format!("\\u{:04x}", c as u32)),
            _ => s.push(c),
        }
    }
    s.push('"');
    s
}
fn wildcard(pattern: &str, name: &str) -> bool {
    let pattern = pattern.to_ascii_uppercase();
    let name = name.to_ascii_uppercase();
    let (p, n) = (pattern.as_bytes(), name.as_bytes());
    let (mut i, mut j, mut star, mut resume) = (0, 0, None, 0);
    while j < n.len() {
        if i < p.len() && (p[i] == b'?' || p[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == b'*' {
            star = Some(i);
            i += 1;
            resume = j;
        } else if let Some(at) = star {
            resume += 1;
            j = resume;
            i = at + 1;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == b'*' {
        i += 1;
    }
    i == p.len()
}
fn portable_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    !name.is_empty()
        && !matches!(name, "." | "..")
        && !name.ends_with(['.', ' '])
        && !name.chars().any(|c| c < ' ' || "<>:\"/\\|?*".contains(c))
        && !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !(stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
}
fn discover(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if path.is_dir() {
        let mut children = fs::read_dir(path)?
            .map(|e| e.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        children.sort();
        for child in children {
            if !child.symlink_metadata()?.file_type().is_symlink() {
                discover(&child, files)?;
            }
        }
    } else if path.is_file() {
        let mut header = [0; 32];
        let count = fs::File::open(path)?.read(&mut header)?;
        let header = &header[..count];
        // Containers and archives are recognised by signature, never by filename.
        if header.starts_with(b"EALIB")
            || esa::has_magic(header)
            || path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("lib"))
        {
            files.push(path.to_path_buf());
        }
    }
    Ok(())
}
/// Turn discovered files into archives, opening any installer container in place.
fn expand(files: Vec<PathBuf>, list: bool) -> Result<Vec<ArchiveSource>> {
    let mut sources = Vec::new();
    for path in files {
        let mut header = [0; 32];
        let count = fs::File::open(&path)?.read(&mut header)?;
        if !esa::has_magic(&header[..count]) {
            sources.push(ArchiveSource { path, entry: None });
            continue;
        }
        let container =
            esa::Container::open(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        if list {
            println!(
                "{}: {} container entries",
                path.display(),
                container.entries().len()
            );
            for entry in container.entries() {
                println!(
                    "  {} [{}] {} {} packed / {} decoded bytes at {}",
                    entry.name,
                    entry.group,
                    if entry.method == esa::Method::Stored {
                        "stored"
                    } else {
                        "DCL"
                    },
                    entry.packed_size,
                    entry.decoded_size,
                    entry.offset
                );
            }
        }
        // Stored entries that are themselves EALIB archives are served without copying.
        for entry in container.entries() {
            if entry.method == esa::Method::Stored && container.archive(&entry.name).is_ok() {
                sources.push(ArchiveSource {
                    path: path.clone(),
                    entry: Some(entry.name.clone()),
                });
            }
        }
    }
    Ok(sources)
}
fn safe_directory(root: &Path, relative: &Path) -> Result<PathBuf> {
    let mut path = root.to_path_buf();
    for part in relative.components() {
        let Component::Normal(part) = part else {
            return Err("unsafe relative output path".into());
        };
        if !portable_name(&part.to_string_lossy()) {
            return Err("archive path is not a portable filename".into());
        }
        path.push(part);
        match path.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                return Err(format!(
                    "output directory is not a regular directory: {}",
                    path.display()
                )
                .into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&path)?,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(path)
}
fn write_resource(path: &Path, bytes: &[u8], overwrite: bool) -> Result<&'static str> {
    let mut exists = false;
    match path.symlink_metadata() {
        Ok(meta) => {
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err("output is not a regular file".into());
            }
            exists = true;
            if meta.len() == bytes.len() as u64 && fs::read(path)? == bytes {
                return Ok("unchanged");
            }
            if !overwrite {
                return Err("output differs; use --overwrite to replace deliberately".into());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let temp = path.with_file_name(format!(
        ".tore-tmp-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        #[cfg(target_os = "windows")]
        if exists {
            fs::remove_file(path)?;
        }
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result?;
    Ok(if exists { "replaced" } else { "written" })
}

fn field_json(
    fields: &std::collections::BTreeMap<String, tore_formats::aircraft::Token>,
) -> String {
    format!(
        "{{{}}}",
        fields
            .iter()
            .map(|(name, t)| format!(
                "{}:{{\"kind\":{},\"value\":{},\"scaled\":{}}}",
                quote(name),
                quote(&t.kind),
                if matches!(t.kind.as_str(), "byte" | "word" | "dword") {
                    t.number().unwrap().to_string()
                } else {
                    quote(&t.value)
                },
                t.scaled
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn analyze(
    name: &str,
    bytes: &[u8],
    available: &std::collections::BTreeSet<String>,
) -> Result<String> {
    use tore_formats::theater::{Environment, Theater};
    if tore_formats::music::resource(name) {
        if name.ends_with(".MUS") {
            let score = tore_formats::music::Score::parse(bytes)?;
            return Ok(format!(
                "{{\"format\":\"FA MUS\",\"pcm_references\":[{}],\"unreachable_bytes\":{},\"missing_pcm\":[{}]}}",
                score
                    .tracks
                    .iter()
                    .map(|n| quote(&score.filename(*n)))
                    .collect::<Vec<_>>()
                    .join(","),
                score.unreachable_bytes,
                score
                    .tracks
                    .iter()
                    .map(|n| score.filename(*n))
                    .filter(|n| !available.contains(n))
                    .map(|n| quote(&n))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        let pcm = tore_formats::pcm::Pcm::parse(name, bytes)?;
        return Ok(format!(
            "{{\"format\":\"PCM8 mono\",\"sample_rate\":{},\"samples\":{},\"duration_seconds\":{}}}",
            pcm.rate,
            pcm.samples.len(),
            pcm.samples.len() as f64 / pcm.rate as f64
        ));
    }
    if matches!(
        name,
        "F18.PT" | "RAFALE.PT" | "F14.PT" | "A4E.PT" | "F31.PT"
    ) {
        let a = tore_formats::aircraft::Aircraft::parse(bytes)?;
        let envelopes = a
            .envelopes
            .iter()
            .map(|e| {
                format!(
                    "{{\"g\":{},\"points_ft_s_ft\":[{}]}}",
                    e.g,
                    e.points
                        .iter()
                        .map(|p| format!("[{},{}]", p[0], p[1]))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let hardpoints=a.hardpoints.iter().map(|h|format!("{{\"flags\":{},\"position_raw\":{:?},\"store\":{},\"count\":{},\"weight_class_raw\":{}}}",h.flags,h.position,h.store.as_ref().map_or("null".into(),|s|quote(s)),h.count,h.weight_class)).collect::<Vec<_>>().join(",");
        return Ok(format!(
            "{{\"format\":\"BRF aircraft\",\"name\":{},\"shape\":{},\"object_fields\":{},\"flight_fields\":{},\"hardpoints\":[{}],\"envelopes\":[{}],\"runtime_parity\":false}}",
            quote(&a.name),
            quote(&a.shape),
            field_json(&a.object),
            field_json(&a.fields),
            hardpoints,
            envelopes
        ));
    }
    if [".JT", ".SEE", ".ECM"]
        .iter()
        .any(|ext| name.ends_with(ext))
    {
        let e = tore_formats::aircraft::Equipment::parse(name, bytes)?;
        if name.ends_with(".JT") {
            tore_formats::weapons::Weapon::parse(name, bytes)?;
        }
        if name.ends_with(".SEE") {
            tore_formats::weapons::Seeker::parse(name, bytes)?;
        }
        if name.ends_with(".ECM") {
            tore_formats::weapons::Countermeasures::parse(name, bytes)?;
        }
        return Ok(format!(
            "{{\"format\":\"BRF equipment\",\"name\":{},\"object_fields\":{},\"equipment_fields\":{}}}",
            quote(&e.name),
            field_json(&e.object),
            field_json(&e.fields)
        ));
    }
    if matches!(name, "F18.SH" | "RAF.SH") {
        let sh = tore_formats::shape::Shape::parse(bytes)?;
        return Ok(format!(
            "{{\"format\":\"SH static projection\",\"faces\":{},\"state_words\":{:?},\"native_vm_parity\":false}}",
            sh.faces.len(),
            sh.state_words.iter().collect::<Vec<_>>()
        ));
    }
    if name.ends_with(".GAS") {
        let tank = tore_formats::weapons::Tank::parse(bytes)?;
        return Ok(format!(
            "{{\"format\":\"BRF tank\",\"empty_weight_lb\":{},\"fuel_weight_lb\":{},\"flags\":{},\"runtime_parity\":false}}",
            tank.empty_weight, tank.fuel_weight, tank.flags
        ));
    }
    if [".JT", ".SEE", ".ECM"]
        .iter()
        .any(|ext| name.ends_with(ext))
    {
        let b = tore_formats::aircraft::Brf::parse(bytes)?;
        return Ok(format!(
            "{{\"format\":\"BRF\",\"blocks\":{},\"statements\":{}}}",
            b.blocks.len(),
            b.blocks.values().map(Vec::len).sum::<usize>()
        ));
    }

    if name.ends_with(".T2") {
        let t = Theater::parse(bytes)?;
        let min = t.cells.iter().map(|c| c.elevation).min().unwrap_or(0);
        let max = t.cells.iter().map(|c| c.elevation).max().unwrap_or(0);
        return Ok(format!(
            "{{\"format\":\"BIT2\",\"name\":{},\"briefing_map\":{},\"cols\":{},\"rows\":{},\"cells_per_tile\":{},\"tile_cols\":{},\"tile_rows\":{},\"cell_offset\":149,\"elevation_byte_range\":[{},{}],\"cell_feet\":8192,\"elevation_step_feet\":256}}",
            quote(&t.name),
            quote(&t.map),
            t.cols,
            t.rows,
            t.cells_per_tile,
            t.tiles[0],
            t.tiles[1],
            min,
            max
        ));
    }
    if name.ends_with(".MM") || name.ends_with(".M") {
        let e = Environment::parse(bytes)?;
        let pair =
            |v: Option<[i32; 2]>| v.map_or("null".into(), |v| format!("[{},{}]", v[0], v[1]));
        let placements = e
            .textures
            .values()
            .map(|p| format!("[{},{},{},{}]", p.col, p.row, p.texture, p.rotation))
            .collect::<Vec<_>>()
            .join(",");
        return Ok(format!(
            "{{\"format\":\"mission-environment\",\"map\":{},\"layer\":{},\"layer_parameter\":{},\"clouds\":{},\"wind_raw\":{},\"time\":{},\"tmap_col_row_texture_rotation\":[{}]}}",
            quote(&e.map),
            quote(&e.layer),
            e.layer_parameter.map_or("null".into(), |v| v.to_string()),
            e.clouds.map_or("null".into(), |v| v.to_string()),
            pair(e.wind),
            pair(e.time),
            placements
        ));
    }
    Ok("null".into())
}

fn extract(options: Options) -> Result<bool> {
    let source = options.source.canonicalize()?;
    let mut files = Vec::new();
    discover(&source, &mut files)?;
    let mut archives = expand(files, options.list)?;
    // Retail discs also bundle other games/installers whose .LIB files are not EALIB.
    // Only directory-based profile discovery skips them; explicit/raw inputs stay strict.
    if source.is_dir()
        && (options.theater.is_some()
            || !options.aircraft.is_empty()
            || options.weapons
            || options.music)
    {
        let mut supported = Vec::new();
        for archive in archives {
            let mut magic = [0; 5];
            let count = fs::File::open(&archive.path)?.read(&mut magic)?;
            if archive.entry.is_some() || (count == 5 && &magic == b"EALIB") {
                supported.push(archive);
            } else {
                eprintln!(
                    "Skipping non-EALIB file in profile scan: {}",
                    archive.path.display()
                );
            }
        }
        archives = supported;
    }
    if archives.is_empty() {
        return Err("No EALIB archives found. Supply a loose archive, an installed/extracted media directory, or a disc folder containing SETUP.ESA; raw ISO images are not supported.".into());
    }
    let source_root = if source.is_dir() {
        source.as_path()
    } else {
        source.parent().ok_or("source has no parent")?
    };
    let planning = options.list || options.dry_run;
    let out = if planning {
        std::path::absolute(&options.out)?
    } else {
        fs::create_dir_all(&options.out)?;
        options.out.canonicalize()?
    };
    if source.is_dir() && out.starts_with(&source) {
        return Err("output directory must be outside the source media tree".into());
    }
    let mut profile_archives = Vec::new();
    let mut profile_paths = Vec::new();
    if !options.aircraft.is_empty() || options.weapons || options.music || options.theater.is_some()
    {
        for source_archive in &archives {
            let relative = source_archive.label(source_root)?;
            if !options
                .exclude_archives
                .iter()
                .any(|p| wildcard(p, &relative))
            {
                match source_archive.open() {
                    Ok(archive) => {
                        profile_archives.push(archive);
                        profile_paths.push(source_archive.display());
                    }
                    Err(error)
                        if !options.aircraft.is_empty() || options.weapons || options.music =>
                    {
                        return Err(error);
                    }
                    Err(_) => {} // The extraction pass below records unsupported archives.
                }
            }
        }
    }
    let available = profile_archives
        .iter()
        .flat_map(|a| a.entries.keys().cloned())
        .collect();
    let dependency_report = tore_formats::aircraft::dependency_report(
        &profile_archives.iter().collect::<Vec<_>>(),
        &options.aircraft,
        options.weapons,
    )?;
    let aircraft_names = &dependency_report.resources;
    let scene_names = if let Some(theater) = &options.theater {
        let layouts: std::collections::BTreeSet<String> = profile_archives
            .iter()
            .flat_map(|a| a.entries.keys())
            .filter(|name| {
                name.ends_with(".MM") && tore_formats::theater::theater_resource(name, theater)
            })
            .cloned()
            .collect();
        tore_formats::mission::scene_dependencies(
            &profile_archives.iter().collect::<Vec<_>>(),
            &layouts.into_iter().collect::<Vec<_>>(),
        )?
    } else {
        Default::default()
    };
    let mut records = Vec::new();
    let mut errors = Vec::new();
    let mut selected = 0;
    let mut destinations = HashSet::new();
    for source_archive in archives {
        let relative_name = source_archive.label(source_root)?;
        if options
            .exclude_archives
            .iter()
            .any(|p| wildcard(p, &relative_name))
        {
            eprintln!("Skipping excluded archive: {relative_name}");
            continue;
        }
        let archive = match source_archive.open() {
            Ok(archive) => archive,
            Err(error) => {
                errors.push(format!("{}: {error}", source_archive.display()));
                continue;
            }
        };
        let relative = &source_archive.relative(source_root)?;
        let mut matched = 0;
        for entry in archive.entries.values() {
            let profile = options.theater.is_some()
                || !options.aircraft.is_empty()
                || options.weapons
                || options.music
                || options.creator;
            let in_profile = (options.creator && tore_formats::ui::creator::resource(&entry.name))
                || (options.music && tore_formats::music::resource(&entry.name))
                || aircraft_names.contains(&entry.name)
                || scene_names.contains(&entry.name)
                || options
                    .theater
                    .as_ref()
                    .is_some_and(|code| tore_formats::theater::theater_resource(&entry.name, code));
            if profile && !in_profile {
                continue;
            }
            if !options.patterns.is_empty()
                && !options.patterns.iter().any(|p| wildcard(p, &entry.name))
            {
                continue;
            }
            matched += 1;
            selected += 1;
            let output = relative.join(&entry.name);
            let mut record = Record {
                archive: source_archive.display(),
                name: entry.name.clone(),
                output: output.to_string_lossy().replace('\\', "/"),
                offset: entry.offset,
                stored: entry.size,
                decoded: 0,
                status: "planned",
                error: String::new(),
                analysis: "null".into(),
                preview: None,
            };
            let result = (|| -> Result<()> {
                if !portable_name(&entry.name) || relative.components().any(|c|!matches!(c,Component::Normal(n) if portable_name(&n.to_string_lossy()))){return Err("resource/archive name is not a portable safe output path".into());}
                if !destinations.insert(record.output.to_ascii_lowercase()) {
                    return Err("output path collides with another archive/resource".into());
                }
                if planning {
                    if options.list {
                        println!(
                            "{} / {} ({} stored bytes, flag {})",
                            relative_name, entry.name, entry.size, entry.flag
                        );
                    }
                    return Ok(());
                }
                let bytes = archive.read_with_limit(&entry.name, options.limit)?;
                record.decoded = bytes.len();
                if profile {
                    record.analysis = analyze(&entry.name, &bytes, &available)?;
                }
                let directory = safe_directory(&out, relative)?;
                record.status =
                    write_resource(&directory.join(&entry.name), &bytes, options.overwrite)?;
                if options.wav_previews
                    && tore_formats::music::resource(&entry.name)
                    && entry.name.ends_with(".11K")
                {
                    let preview = format!("{}.wav", entry.name);
                    let relative_preview =
                        relative.join(&preview).to_string_lossy().replace('\\', "/");
                    if !destinations.insert(relative_preview.to_ascii_lowercase()) {
                        return Err("WAV output collision".into());
                    }
                    let pcm = tore_formats::pcm::Pcm::parse(&entry.name, &bytes)?;
                    write_resource(&directory.join(preview), &pcm.wav(), options.overwrite)?;
                    record.preview = Some(relative_preview);
                }
                Ok(())
            })();
            if let Err(error) = result {
                record.status = "error";
                record.error = error.to_string();
            }
            records.push(record);
        }
        println!(
            "{relative_name}: {matched} selected / {} unique resources",
            archive.entries.len()
        );
    }
    if selected == 0 {
        errors.push("No resources matched the requested filters".into());
    }
    let failed = records.iter().filter(|r| r.status == "error").count() + errors.len();
    for error in &errors {
        eprintln!("{error}");
    }
    for record in records.iter().filter(|r| r.status == "error") {
        eprintln!("{}/{}: {}", record.archive, record.name, record.error);
    }
    if !planning {
        let entries=records.iter().map(|r|format!("{{\"archive\":{},\"name\":{},\"output\":{},\"offset\":{},\"stored_bytes\":{},\"decoded_bytes\":{},\"status\":{},\"error\":{},\"analysis\":{},\"preview_output\":{}}}",quote(&r.archive),quote(&r.name),quote(&r.output),r.offset,r.stored,r.decoded,quote(r.status),quote(&r.error),r.analysis,r.preview.as_ref().map_or("null".into(), |p| quote(p)))).collect::<Vec<_>>().join(",\n");
        let edges = dependency_report
            .edges
            .iter()
            .map(|e| {
                format!(
                    "{{\"source\":{},\"target\":{},\"kind\":{},\"available\":{},\"included\":{}}}",
                    quote(&e.source),
                    quote(&e.target),
                    quote(e.kind),
                    e.available,
                    records.iter().any(|r| r.name == e.target
                        && matches!(r.status, "written" | "unchanged" | "replaced"))
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let providers = dependency_report
            .providers
            .iter()
            .map(|(name, indices)| {
                format!(
                    "{{\"name\":{},\"archives\":[{}],\"selected_archive\":{}}}",
                    quote(name),
                    indices
                        .iter()
                        .map(|&i| quote(&profile_paths[i]))
                        .collect::<Vec<_>>()
                        .join(","),
                    indices
                        .last()
                        .map_or("null".into(), |&i| quote(&profile_paths[i]))
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let dependency_json = format!(
            "{{\"method\":\"reviewed roots and literal candidates; not complete native coverage\",\"filtered\":{},\"native_parity\":false,\"edges\":[{}],\"providers\":[{}]}}",
            !options.patterns.is_empty(),
            edges,
            providers
        );
        let report = format!(
            "{{\"schema_version\":1,\"source\":{},\"output_root\":{},\"complete\":{},\"selected\":{},\"errors\":[{}],\"entries\":[{}],\"dependencies\":{}}}\n",
            quote(&source.to_string_lossy()),
            quote(&out.to_string_lossy()),
            failed == 0,
            selected,
            errors
                .iter()
                .map(|e| quote(e))
                .collect::<Vec<_>>()
                .join(","),
            entries,
            dependency_json
        );
        write_resource(&out.join("extraction-report.json"), report.as_bytes(), true)?;
        println!("Report: {}", out.join("extraction-report.json").display());
    }
    println!(
        "{}: {selected} resources, {failed} errors",
        if planning {
            "Plan (no output written)"
        } else {
            "Extraction"
        }
    );
    Ok(failed == 0)
}
fn main() -> Result<()> {
    let mut options = Options {
        source: PathBuf::new(),
        out: PathBuf::from(".local/extracted"),
        patterns: vec![],
        exclude_archives: vec![],
        theater: None,
        aircraft: Vec::new(),
        weapons: false,
        music: false,
        creator: false,
        wav_previews: false,
        list: false,
        dry_run: false,
        overwrite: false,
        limit: 256 * 1024 * 1024,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--source" => {
                options.source = PathBuf::from(args.next().ok_or("--source needs a path")?)
            }
            "--out" => options.out = PathBuf::from(args.next().ok_or("--out needs a path")?),
            "--include" => options
                .patterns
                .push(args.next().ok_or("--include needs a glob")?),
            "--exclude-archive" => options.exclude_archives.push(
                args.next()
                    .ok_or("--exclude-archive needs a source-relative glob")?,
            ),
            "--aircraft" => {
                options
                    .aircraft
                    .push(tore_formats::aircraft::AircraftId::parse(
                        &args
                            .next()
                            .ok_or("--aircraft needs a supported aircraft ID (see --help)")?,
                    )?);
            }
            "--weapons" => options.weapons = true,
            "--music" => options.music = true,
            "--creator" => options.creator = true,
            "--wav-previews" => options.wav_previews = true,
            "--theater" => {
                let code = args
                    .next()
                    .ok_or("--theater needs a code or all")?
                    .to_ascii_uppercase();
                if code != "ALL"
                    && !tore_formats::theater::THEATERS
                        .iter()
                        .any(|(id, _)| *id == code)
                {
                    return Err(format!(
                        "Unknown theater {code}; use all or one of: {}",
                        tore_formats::theater::THEATERS
                            .iter()
                            .map(|(id, _)| *id)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                    .into());
                }
                options.theater = Some(code);
            }
            "--list" => options.list = true,
            "--dry-run" => options.dry_run = true,
            "--overwrite" => options.overwrite = true,
            "--max-entry-mib" => {
                let mib: usize = args
                    .next()
                    .ok_or("--max-entry-mib needs a number")?
                    .parse()?;
                if !(1..=1024).contains(&mib) {
                    return Err("entry limit must be 1..1024 MiB".into());
                }
                options.limit = mib * 1024 * 1024;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: tore-extract --source FILE_OR_DIRECTORY [--out DIRECTORY] [--aircraft f18|rafale|f14|a4e|x31|mig29|su27|mig21|su25|mig23|su35|f22|f22n] [--weapons] [--music] [--creator] [--wav-previews] [--theater CODE|all] [--include GLOB] [--exclude-archive GLOB] [--list | --dry-run] [--overwrite] [--max-entry-mib N]\n\nRecursively discovers EALIB archives by signature, independent of game/archive names.\n--source may also name a disc folder or a SETUP.ESA installer container, recognised\nby its signature; archives stored inside it are read in place and keep their\ncontainer provenance (SETUP.ESA:FA_1.LIB). --list also prints the container directory.\nExtracts stored and raw-literal DCL entries. Source files remain untouched.\nFilters match resource names case-insensitively (* and ?), and may repeat.\nExisting identical files are reused; differing files require --overwrite.\nOutput preserves source hierarchy/archive names. No resource code is executed.\nRaw ISO images, coded-literal DCL, and general format conversion are not implemented. --music --wav-previews adds lossless PCM WAV wrappers.\nUse tools/extract_assets.py for the portable entry point and SHA-256 report hashes."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    if options.source.as_os_str().is_empty() {
        return Err("--source is required; use --help".into());
    }
    if options.wav_previews && !options.music {
        return Err("--wav-previews requires --music".into());
    }
    if !extract(options)? {
        std::process::exit(1);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_are_case_insensitive_and_backtrack() {
        assert!(wildcard("CHOOSE*.PIC", "choosev.pic"));
        assert!(wildcard("A*B?", "AXYB1"));
        assert!(!wildcard("*.PIC", "MODEL.SH"));
        assert!(!wildcard("A?", "ABC"));
    }
    #[test]
    fn portable_paths_reject_traversal_and_windows_device_names() {
        for bad in [
            "../a", "a/b", "a\\b", "C:DATA", "CON", "lpt1.pic", "file.", "NUL.txt",
        ] {
            assert!(!portable_name(bad), "{bad}");
        }
        for good in ["&CLICK.11K", "^MF.11K", "$F14.PIC", "FA_1.LIB"] {
            assert!(portable_name(good));
        }
    }
    #[test]
    fn json_escaping() {
        assert_eq!(quote("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
    }
}
