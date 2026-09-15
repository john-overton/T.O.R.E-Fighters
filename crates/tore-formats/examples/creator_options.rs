//! Shared CLI/app table reader. Only derived inert lists are written.
use std::{fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: creator_options FA.EXE OUTPUT".into());
    }
    if fs::metadata(&args[0])?.len() > 16 * 1024 * 1024 {
        return Err("input too large".into());
    }
    let tables = tore_formats::ui::creator::Options::parse(&fs::read(&args[0])?)?;
    let bytes = tables.encode();
    tore_formats::ui::creator::Options::decode(&bytes)?;
    use std::io::Write;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?
        .write_all(&bytes)?;
    println!("Recovered 30 creator fields and 16 target lists");
    Ok(())
}
