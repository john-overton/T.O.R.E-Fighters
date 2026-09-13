//! Generic EALIB extraction. No graphics, audio, Python, or retail fixtures required.
use std::{
    collections::HashSet,
    error::Error,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use tore_formats::Archive;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
struct Options {
    source: PathBuf,
    out: PathBuf,
    patterns: Vec<String>,
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
        let mut header = [0; 5];
        let count = fs::File::open(path)?.read(&mut header)?;
        if (count == 5 && &header == b"EALIB")
            || path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("lib"))
        {
            files.push(path.to_path_buf());
        }
    }
    Ok(())
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
fn extract(options: Options) -> Result<bool> {
    let source = options.source.canonicalize()?;
    let mut archives = Vec::new();
    discover(&source, &mut archives)?;
    if archives.is_empty() {
        return Err("No EALIB archives found. Supply a loose archive or an installed/extracted media directory; ISO/ESA containers are not supported yet.".into());
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
    let mut records = Vec::new();
    let mut errors = Vec::new();
    let mut selected = 0;
    let mut destinations = HashSet::new();
    for path in archives {
        let archive = match Archive::open(&path) {
            Ok(archive) => archive,
            Err(error) => {
                errors.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        let relative = path.strip_prefix(source_root)?;
        let mut matched = 0;
        for entry in archive.entries.values() {
            if !options.patterns.is_empty()
                && !options.patterns.iter().any(|p| wildcard(p, &entry.name))
            {
                continue;
            }
            matched += 1;
            selected += 1;
            let output = relative.join(&entry.name);
            let mut record = Record {
                archive: path.to_string_lossy().into_owned(),
                name: entry.name.clone(),
                output: output.to_string_lossy().replace('\\', "/"),
                offset: entry.offset,
                stored: entry.size,
                decoded: 0,
                status: "planned",
                error: String::new(),
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
                            relative.display(),
                            entry.name,
                            entry.size,
                            entry.flag
                        );
                    }
                    return Ok(());
                }
                let bytes = archive.read_with_limit(&entry.name, options.limit)?;
                record.decoded = bytes.len();
                let directory = safe_directory(&out, relative)?;
                record.status =
                    write_resource(&directory.join(&entry.name), &bytes, options.overwrite)?;
                Ok(())
            })();
            if let Err(error) = result {
                record.status = "error";
                record.error = error.to_string();
            }
            records.push(record);
        }
        println!(
            "{}: {matched} selected / {} unique resources",
            relative.display(),
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
        let entries=records.iter().map(|r|format!("{{\"archive\":{},\"name\":{},\"output\":{},\"offset\":{},\"stored_bytes\":{},\"decoded_bytes\":{},\"status\":{},\"error\":{}}}",quote(&r.archive),quote(&r.name),quote(&r.output),r.offset,r.stored,r.decoded,quote(r.status),quote(&r.error))).collect::<Vec<_>>().join(",\n");
        let report = format!(
            "{{\"schema_version\":1,\"source\":{},\"output_root\":{},\"complete\":{},\"selected\":{},\"errors\":[{}],\"entries\":[{}]}}\n",
            quote(&source.to_string_lossy()),
            quote(&out.to_string_lossy()),
            failed == 0,
            selected,
            errors
                .iter()
                .map(|e| quote(e))
                .collect::<Vec<_>>()
                .join(","),
            entries
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
                    "Usage: tore-extract --source FILE_OR_DIRECTORY [--out DIRECTORY] [--include GLOB] [--list | --dry-run] [--overwrite] [--max-entry-mib N]\n\nRecursively discovers EALIB archives by signature, independent of game/archive names.\nExtracts stored and raw-literal DCL entries. Source files remain untouched.\nFilters match resource names case-insensitively (* and ?), and may repeat.\nExisting identical files are reused; differing files require --overwrite.\nOutput preserves source hierarchy/archive names. No resource code is executed.\nISO, ESA installers, coded-literal DCL, and format conversion are not implemented.\nUse tools/extract_assets.py for the portable entry point and SHA-256 report hashes."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    if options.source.as_os_str().is_empty() {
        return Err("--source is required; use --help".into());
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
