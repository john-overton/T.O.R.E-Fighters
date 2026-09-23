//! Inspect inert shape geometry and state branches from user-owned media.
use std::{collections::BTreeMap, env, fs::File, io::Read};
use tore_formats::{module, shape::Shape};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: shape_inspect FILE.SH [--branches] [HEX_WORD=VALUE ...]")?;
    let mut data = Vec::new();
    File::open(path)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut data)?;
    let mut state = BTreeMap::new();
    let mut branches = false;
    for arg in args {
        if arg == "--branches" {
            branches = true;
            continue;
        }
        let (word, value) = arg.split_once('=').ok_or("expected HEX_WORD=VALUE")?;
        state.insert(
            usize::from_str_radix(word.trim_start_matches("0x"), 16)?,
            value.parse()?,
        );
    }
    println!(
        "streamer={:?}",
        tore_formats::shape::StreamerDef::parse(&data)?
    );
    let shape = if branches {
        Shape::with_export_state(&data, &state)?
    } else {
        Shape::with_state(&data, &state)?
    };
    println!(
        "code={} faces={} lines={} words={:x?}",
        module::code(&data)?.0.len(),
        shape.faces.len(),
        shape.lines.len(),
        shape.state_words
    );
    for f in shape.faces {
        println!(
            "{:x} subtype={:x} texture={} positions={:?} uv={:?}",
            f.address, f.subtype, f.texture, f.positions, f.uv
        );
    }
    Ok(())
}
