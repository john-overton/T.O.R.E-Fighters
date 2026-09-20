//! Inert diagnostic for an extracted MM plus colocated OT/SH resources.
use std::{collections::BTreeSet, env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        env::args_os()
            .nth(1)
            .ok_or("usage: airport_inspect LAYOUT.MM RESOURCE_DIR")?,
    );
    let root = PathBuf::from(
        env::args_os()
            .nth(2)
            .ok_or("usage: airport_inspect LAYOUT.MM RESOURCE_DIR")?,
    );
    let layout = tore_formats::mission::Layout::parse(
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or("non-UTF8 layout name")?,
        &fs::read(&path)?,
    )?;
    let types: BTreeSet<_> = layout.placements.iter().map(|p| &p.object_type).collect();
    let mut shapes = BTreeSet::new();
    for object_type in &types {
        let definition =
            tore_formats::static_object::Definition::parse(&fs::read(root.join(object_type))?)?;
        if let Some(shape) = definition.main_shape {
            tore_formats::shape::Shape::parse(&fs::read(root.join(&shape))?)?;
            shapes.insert(shape);
        }
    }
    println!(
        "{}: {} placements, {} definitions, {} shapes",
        layout.resource,
        layout.placements.len(),
        types.len(),
        shapes.len()
    );
    Ok(())
}
