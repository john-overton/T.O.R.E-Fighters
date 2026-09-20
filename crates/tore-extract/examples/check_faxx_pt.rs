//! Validate exported PT identity and the single hook capability change.
use std::{error::Error, fs};
use tore_formats::aircraft::Brf;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(3..=4).contains(&args.len()) {
        return Err("usage: check_faxx_pt DONOR.PT EXPORTED.PT [FAXX|F22]".into());
    }
    let donor = Brf::parse(&fs::read(&args[1])?)?;
    let exported = Brf::parse(&fs::read(&args[2])?)?;
    let independent = match args.get(3).map(String::as_str).unwrap_or("FAXX") {
        "FAXX" => true,
        "F22" => false,
        _ => return Err("unsupported export identity".into()),
    };
    if independent {
        for (block, expected) in [
            ("ot_names", vec!["F/A-XX", "F/A-XX Concept", "FAXX.PT"]),
            ("shape", vec!["FAXX.SH"]),
            ("shadowShape", vec!["FAXX_S.SH"]),
        ] {
            if exported.strings(block)? != expected {
                return Err(format!("unexpected identity block {block}").into());
            }
        }
    }
    if donor.blocks.keys().ne(exported.blocks.keys()) {
        return Err("BRF block set changed".into());
    }
    let mut hook_checked = false;
    for (label, tokens) in &donor.blocks {
        if independent && matches!(label.as_str(), "ot_names" | "shape" | "shadowShape") {
            continue;
        }
        let other = &exported.blocks[label];
        if tokens.len() != other.len() {
            return Err(format!("BRF block length changed: {label}").into());
        }
        let mut offset = 0;
        for (before, after) in tokens.iter().zip(other) {
            if label.is_empty() && offset == 0xba {
                if before.kind != "dword"
                    || after.kind != "dword"
                    || before.scaled
                    || after.scaled
                    || before.number()? != 0x91
                    || after.number()? != 0x93
                {
                    return Err("expected only hook bit 0x02 added at PT+0xba".into());
                }
                hook_checked = true;
            } else if before.kind != after.kind
                || before.value != after.value
                || before.scaled != after.scaled
            {
                return Err(format!("unexpected donor field change in block {label:?}").into());
            }
            if label.is_empty() {
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
    println!("PT identity, enabled hook, and unchanged remaining donor fields verified.");
    Ok(())
}
