//! Inert PIC dimensions and palette diagnostic.
use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in env::args().skip(1) {
        let pic = tore_formats::Pic::parse(&fs::read(&path)?)?;
        println!(
            "{path}: {}x{} palette={} opaque={}",
            pic.width,
            pic.height,
            pic.palette.len(),
            pic.mask.iter().filter(|visible| **visible).count()
        );
    }
    Ok(())
}
