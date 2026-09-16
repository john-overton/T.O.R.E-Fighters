//! Inspect STRIP's required shape points; does not place or activate a runway.
use std::{collections::BTreeSet, env, fs};
use tore_formats::shape::{self, Shape};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if !(1..=3).contains(&args.len()) {
        return Err("usage: native_strip RUNWAY.SH [STRIP.OT [ISOLATED-PLACEMENT]]".into());
    }
    if let Some(path) = args.get(2) {
        let placement = tore_formats::strip::Placement::parse(&fs::read(path)?)?;
        println!(
            "{path}: fixed8={:?}, PA={:?}, raw_nationality={}, source_flags={:#x}, speed_fixed8={}, alias={}, native_name_bytes={:?}; placement inputs only",
            placement.position_fixed8(),
            placement.angles_pa(),
            placement.nationality,
            placement.flags,
            placement.speed_fixed8(),
            placement.alias,
            placement.native_name()
        );
    }
    if let Some(path) = args.get(1) {
        let definition = tore_formats::strip::Definition::parse(&fs::read(path)?)?;
        let shape_name = std::path::Path::new(&args[0])
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("invalid shape filename")?;
        if !shape_name.eq_ignore_ascii_case(&definition.shape) {
            return Err("shape filename does not match the STRIP definition reference".into());
        }
        println!(
            "{path}: shape={}, flags={:#x}; definition metadata only",
            definition.shape, definition.flags
        );
    }
    let bytes = fs::read(&args[0])?;
    let boxes = shape::contact_boxes(&bytes)?.ok_or("shape has no F2 box list")?;
    println!("{}: {} contact boxes", args[0], boxes.len());
    // Reviewed STRIPAddProc call order, not a general airport naming convention.
    for id in [
        0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x11, 0x17, 0x12, 0x18,
    ] {
        let b = boxes
            .iter()
            .find(|b| b.id == id)
            .ok_or_else(|| format!("STRIP initialization requires absent contact box {id:#x}"))?;
        let orientation = matches!(id, 0x17 | 0x18);
        let words = if orientation {
            b.pairs.map(|pair| pair[0])
        } else {
            b.midpoint()
        };
        println!(
            "box {id:#04x}: flags={:#04x}, words={words:?}, orientation_record={orientation}",
            b.flags,
        );
    }
    // This projector is explicitly incomplete. Success is not all-LOD/resource closure.
    match Shape::parse(&bytes) {
        Ok(shape) => {
            let textures: BTreeSet<_> = shape
                .faces
                .iter()
                .filter_map(|f| (!f.texture.is_empty()).then_some(f.texture.as_str()))
                .collect();
            println!(
                "partial static projection: {} faces; textures={textures:?}; state_words={:?}",
                shape.faces.len(),
                shape.state_words
            );
        }
        Err(error) => println!("static projection unsupported: {error}"),
    }
    println!(
        "Diagnostic only: world initialization, full drawing closure and collision acceptance remain required."
    );
    Ok(())
}
