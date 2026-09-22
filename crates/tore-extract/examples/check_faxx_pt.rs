//! Validate exported PT identity against the F-22N donor, whose hook bit is already set.
use std::{error::Error, fs};
use tore_formats::aircraft::Brf;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: check_faxx_pt DONOR.PT EXPORTED.PT".into());
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
    let mut hook_checked = false;
    for (label, tokens) in &donor.blocks {
        if matches!(label.as_str(), "ot_names" | "shape" | "shadowShape") {
            continue;
        }
        let other = &exported.blocks[label];
        if tokens.len() != other.len() {
            return Err(format!("BRF block length changed: {label}").into());
        }
        let mut offset = 0;
        for (before, after) in tokens.iter().zip(other) {
            if before.kind != after.kind
                || before.value != after.value
                || before.scaled != after.scaled
            {
                return Err(format!("unexpected donor field change in block {label:?}").into());
            }
            if label.is_empty() {
                if offset == 0xba {
                    if before.kind != "dword" || before.scaled || before.number()? != 0xd3 {
                        return Err("donor PT+0xba is not the hook-capable dword $d3".into());
                    }
                    hook_checked = true;
                }
                offset += match before.kind.as_str() {
                    "byte" => 1,
                    "word" => 2,
                    "dword" | "ptr" | "symbol" => 4,
                    _ => return Err("unreviewed root field width".into()),
                };
            }
        }
    }
    if !hook_checked {
        return Err("missing plane capability field".into());
    }
    println!("PT identity, donor hook capability $d3, and unchanged donor fields verified.");
    Ok(())
}
