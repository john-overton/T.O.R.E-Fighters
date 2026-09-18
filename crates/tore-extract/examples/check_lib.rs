//! Independent archive validation for export tools; failures return nonzero.
use std::{error::Error, fs, path::Path};
use tore_formats::Archive;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 {
        return Err("usage: check_lib ARCHIVE.LIB EXPECTED_FILE...".into());
    }
    let archive = Archive::open(&args[1])?;
    if archive.entries.len() != args.len() - 2 {
        return Err("archive entry count differs from supplied files".into());
    }
    for path in &args[2..] {
        let name = Path::new(path)
            .file_name()
            .ok_or("no filename")?
            .to_str()
            .ok_or("non-UTF8 name")?;
        if archive.read(name)? != fs::read(path)? {
            return Err(format!("archive payload mismatch: {name}").into());
        }
    }
    println!(
        "Independent archive reader verified {} entries and payloads.",
        archive.entries.len()
    );
    Ok(())
}
