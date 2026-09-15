//! Local geometry diagnostic: imported inputs, no world/cache or flight activation.
use std::{env, fs};
use tore_formats::{
    flight_model::terrain_contact::{SqrtTable, vertical_cell},
    shape,
    theater::Theater,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("usage: native_land_geometry SQRT_TABLE T2 [SH ...]".into());
    }
    let table = SqrtTable::parse(&fs::read(&args[0])?)?;
    let terrain = Theater::parse(&fs::read(&args[1])?)?;
    // The native traversal's signed fixed8 theater extent must be representable.
    if terrain.cols >= 1024 || terrain.rows >= 1024 {
        return Err("diagnostic theater extent exceeds signed fixed8 domain".into());
    }
    let mut queries = 0usize;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    let mut sloped = 0usize;
    for row in 0..terrain.rows {
        for col in 0..terrain.cols {
            let heights = [
                (col, row),
                (col + 1, row),
                (col, row + 1),
                (col + 1, row + 1),
            ]
            // 0x4c608d returns the source fallback cell with elevation zero.
            .map(|(x, z)| {
                terrain
                    .lookup(x as i32, z as i32, 0)
                    .map_or(0, |c| c.elevation)
            });
            for [dx, dz] in [
                [0, 0],
                [1 << 20, 1 << 20],
                [(1 << 21) - 1, 0],
                [0, (1 << 21) - 1],
            ] {
                let x = ((col as i32) << 21) + dx;
                let z = ((row as i32) << 21) + dz;
                let run = || {
                    vertical_cell(
                        &table,
                        [col as i16, row as i16],
                        heights,
                        [x, 30000 * 256, z],
                        [x, -100 * 256, z],
                        0,
                    )
                };
                let first = run()?;
                assert_eq!(first, run()?);
                let hit = first.ok_or("expected terrain hit in diagnostic vertical segment")?;
                min_y = min_y.min(hit.position[1]);
                max_y = max_y.max(hit.position[1]);
                sloped += usize::from(hit.normal[0] != 0 || hit.normal[2] != 0);
                queries += 1;
            }
        }
    }
    println!(
        "cell geometry: {queries} cases, repeated identically; sloped={sloped}; Y fixed8={min_y}..{max_y}"
    );
    for path in &args[2..] {
        println!(
            "shape {path}: contact offset {:?} (None uses native zero fallback)",
            shape::contact_offset(&fs::read(path)?)?
        );
    }
    println!(
        "Diagnostic only: no cache, object dispatcher, runway placement, live contact or retail comparison."
    );
    Ok(())
}
