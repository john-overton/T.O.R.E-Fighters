//! Inert SH projection for local export tools. No imported machine code is executed.
use std::{collections::BTreeMap, error::Error};
use tore_formats::shape::Shape;
fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args().nth(1).ok_or("usage: shape_json FILE.SH")?;
    let data = std::fs::read(path)?;
    let mut state = BTreeMap::new();
    for arg in std::env::args().skip(2) {
        let (address, value) = arg.split_once('=').ok_or("expected HEX_ADDRESS=VALUE")?;
        state.insert(
            usize::from_str_radix(address.trim_start_matches("0x"), 16)?,
            value.parse()?,
        );
    }
    let shape = if std::env::var_os("TORE_EXPORT_BRANCHES").is_some() {
        Shape::with_export_state(&data, &state)?
    } else {
        Shape::with_state(&data, &state)?
    };
    println!("[");
    for (i, f) in shape.faces.iter().enumerate() {
        if i > 0 {
            println!(",");
        }
        print!(
            "{{\"address\":{},\"positions\":{:?},\"uv\":{:?},\"colors\":{:?}}}",
            f.address, f.positions, f.uv, f.colors
        );
    }
    println!("\n]");
    Ok(())
}
