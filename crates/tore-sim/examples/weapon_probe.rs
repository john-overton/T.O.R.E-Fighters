//! Read user-extracted JT files and exercise recovered scalar components.
//! This is deliberately not a complete projectile or engagement simulator.
use std::{env, fs::File, io::Read, path::Path};
use tore_formats::weapons::Weapon;
use tore_sim::combat::{EnginePhase, commanded_speed, engine_phase, launch_speed, removal_due};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = env::args().skip(1).collect();
    if paths.is_empty() || paths.len() > 4096 {
        return Err("usage: weapon_probe EXTRACTED.JT [MORE.JT ...] (at most 4096)".into());
    }
    println!("diagnostic components only; native lifecycle parity=false");
    println!("source,launcher_fps,launch_fps,powered_sea_fps,powered_19968ft_fps,coast_fps");
    for path in paths {
        let mut bytes = Vec::new();
        File::open(&path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        let name = Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("invalid filename")?;
        let w = Weapon::parse(name, &bytes)?;
        let m = &w.movement;
        // Native consumers intentionally differ in their signed age handling.
        // Probe the reviewed unsigned lifetime boundary, without inventing a clock.
        assert!(removal_due(m, m.remove_t, 0, 0));
        if m.remove_t != 0 {
            assert!(!removal_due(m, m.remove_t - 1, 0, 0));
        }
        if m.ignite_t < m.fuel_t && m.ignite_t <= i16::MAX as u16 {
            assert_eq!(engine_phase(m, m.ignite_t, 0), EnginePhase::Powered);
        }
        for launcher in [0, 300, 600, 1200] {
            let speed = launch_speed(m, launcher * 256)?;
            println!(
                "{},{launcher},{speed},{},{},{}",
                w.source,
                commanded_speed(m, EnginePhase::Powered, speed * 256, 0),
                commanded_speed(m, EnginePhase::Powered, speed * 256, 78 << 16),
                commanded_speed(m, EnginePhase::Coast, speed * 256, 0)
            );
        }
    }
    Ok(())
}
