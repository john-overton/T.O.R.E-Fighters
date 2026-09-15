//! Print static DLG draw records; dynamic runtime geometry is a separate contract.
use std::{env, fs::File, io::Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = env::args_os().skip(1).collect();
    if paths.is_empty() {
        return Err("usage: dialog_geometry EXTRACTED.DLG ...".into());
    }
    for path in paths {
        let mut data = Vec::new();
        File::open(&path)?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut data)?;
        println!(
            "{}\n{:#?}",
            std::path::Path::new(&path).display(),
            tore_formats::ui::dialog::parse(&data)?
        );
    }
    Ok(())
}
