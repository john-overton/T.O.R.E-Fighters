//! Validate a separate original-game PT without changing the simulator's aircraft catalog.
use std::{error::Error, fs};
use tore_formats::aircraft::Brf;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: check_faxx_pt DONOR.PT FAXX.PT".into());
    }
    let donor = Brf::parse(&fs::read(&args[1])?)?;
    let exported = Brf::parse(&fs::read(&args[2])?)?;
    for (block, expected) in [
        ("ot_names", vec!["F/A-XX", "F/A-XX Concept", "FAXX.PT"]),
        ("shape", vec!["FAXX.SH"]),
        ("shadowShape", vec!["FAXX_S.SH"]),
    ] {
        if exported.strings(block)? != expected {
            return Err(format!("unexpected identity block {block}").into());
        }
    }
    if donor.blocks.keys().ne(exported.blocks.keys()) {
        return Err("BRF block set changed".into());
    }
    for (label, tokens) in &donor.blocks {
        if matches!(label.as_str(), "ot_names" | "shape" | "shadowShape") {
            continue;
        }
        let signature =
            |t: &tore_formats::aircraft::Token| (t.kind.clone(), t.value.clone(), t.scaled);
        if tokens
            .iter()
            .map(signature)
            .ne(exported.blocks[label].iter().map(signature))
        {
            return Err(format!("donor field changed in block {label:?}").into());
        }
    }
    println!("Independent PT identity and unchanged donor settings verified.");
    Ok(())
}
