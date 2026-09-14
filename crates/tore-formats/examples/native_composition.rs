//! Diagnostic only: imported static tables, never imported code execution.
use std::{env, fs::File, io::Read};
use tore_formats::flight_model::{
    clock_rng::{FixedClock, NativeRng},
    integration::{MovementAngles, Velocity},
    rotation::{AtanTable, TrigTable, cockpit_angles, world_velocity},
};
fn bounded(path: &str, size: usize) -> std::io::Result<Vec<u8>> {
    let mut data = Vec::new();
    File::open(path)?
        .take(size as u64 + 1)
        .read_to_end(&mut data)?;
    Ok(data)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: native_composition SINE-Q15.BIN ATAN-PA.BIN".into());
    }
    let t = TrigTable::parse(&bounded(&args[1], 642)?)?;
    let a = AtanTable::parse(&bounded(&args[2], 1028)?)?;
    let mut clock = FixedClock::default();
    let mut rng = NativeRng::seeded(1)?;
    let mut elapsed = 0;
    for tick in 0..120 {
        elapsed += clock.advance(false) as i32;
        if tick % 30 == 0 {
            let m = MovementAngles {
                heading: 0,
                pitch: tick * 256,
                roll: 30 * 256,
            };
            let velocity = world_velocity(
                &t,
                Velocity {
                    forward: 500 * 256,
                    side: 10 * 256,
                    down: 5 * 256,
                },
                m,
                m.pitch,
            )?;
            let (body, flip) = cockpit_angles(&t, &a, m, [2 * 256, 5 * 256, 0], [0, 0], 0)?;
            println!(
                "tick={tick} elapsed={elapsed} world={velocity:?} cockpit_pa={body:?} heading_flip={flip} rng={}",
                rng.below(256)?
            );
        }
    }
    assert_eq!(elapsed, 256);
    println!("120 fixed steps = {elapsed} native time units; diagnostic, not whole-tick parity");
    Ok(())
}
