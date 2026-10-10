//! Surveys the aircraft the Quick Mission ground-target templates park, from
//! user-owned `FA_2.LIB` and `FA_1.LIB`: each PT's OBJECT block, its shape's
//! state words and, for each word set to 1 alone, how many faces it adds and
//! how low they reach. A gear word adds the wheels, the lowest faces of the
//! shape. Read-only; nothing is written.
//!
//! ```text
//! parked_inspect FA_2.LIB FA_1.LIB [PT ...]
//! ```
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use tore_formats::{
    Archive,
    parked_aircraft::ParkedType,
    quick_template::{ObjectKind, Template},
    shape::{Shape, contact_offset, object_scale},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: parked_inspect FA_2.LIB FA_1.LIB [PT ...]";
    let libs = [
        Archive::open(PathBuf::from(args.first().ok_or(usage)?))?,
        Archive::open(PathBuf::from(args.get(1).ok_or(usage)?))?,
    ];
    let read = |name: &str| {
        libs.iter()
            .find_map(|lib| lib.read(name).ok())
            .ok_or_else(|| format!("{name} is in neither archive"))
    };
    let mut types: BTreeSet<String> = args[2..].iter().map(|a| a.to_ascii_uppercase()).collect();
    if types.is_empty() {
        for name in libs[0]
            .entries
            .keys()
            .filter(|n| n.starts_with("~Q") && n.ends_with(".M"))
        {
            let Ok(template) = Template::parse(name, &read(name)?) else {
                continue;
            };
            for object in &template.objects {
                if let ObjectKind::Named(named) = &object.kind {
                    let upper = named.to_ascii_uppercase();
                    let pt = if upper.contains('.') {
                        upper
                    } else {
                        format!("{upper}.PT")
                    };
                    if pt.ends_with(".PT") && read(&pt).is_ok() {
                        types.insert(pt);
                    }
                }
            }
        }
    }
    for pt in &types {
        let parked = ParkedType::parse(&read(pt)?)?;
        let bytes = read(&parked.shape)?;
        let scale = object_scale(&bytes)?;
        let contact = contact_offset(&bytes)?;
        let neutral = match Shape::with_state(&bytes, &BTreeMap::new()) {
            Ok(shape) => shape,
            Err(error) => {
                println!("{pt} shape {} DOES NOT READ: {error}", parked.shape);
                continue;
            }
        };
        let low = |shape: &Shape| {
            shape
                .faces
                .iter()
                .flat_map(|f| &f.positions)
                .map(|p| p[2])
                .fold(f32::INFINITY, f32::min)
        };
        println!(
            "{pt} {:?} shape {} scale {scale} class {:#06x} hp {} exp {} crater {} contact {:?} faces {} low {} words {:x?} debris dmg {:?} dst {:?}",
            parked.short_name,
            parked.shape,
            parked.class,
            parked.hit_points,
            parked.explosion,
            parked.crater,
            contact,
            neutral.faces.len(),
            low(&neutral),
            neutral.state_words,
            parked.debris_damaged,
            parked.debris_destroyed,
        );
        for word in &neutral.state_words {
            match Shape::with_state(&bytes, &BTreeMap::from([(*word, 1)])) {
                Ok(shape) => {
                    let known: BTreeSet<usize> = neutral.faces.iter().map(|f| f.address).collect();
                    let added: Vec<_> = shape
                        .faces
                        .iter()
                        .filter(|f| !known.contains(&f.address))
                        .collect();
                    let added_low = added
                        .iter()
                        .flat_map(|f| &f.positions)
                        .map(|p| p[2])
                        .fold(f32::INFINITY, f32::min);
                    println!(
                        "  word {word:#x}=1: faces {} (+{} new, low {added_low}) shape low {}",
                        shape.faces.len(),
                        added.len(),
                        low(&shape)
                    );
                }
                Err(error) => println!("  word {word:#x}=1: DOES NOT READ: {error}"),
            }
        }
    }
    Ok(())
}
