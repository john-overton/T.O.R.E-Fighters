//! `--surface-drive THEATER STEM [--surface-seed N] [--at SECONDS,...]
//! [--kill ORDINAL@SECONDS ...]`: a development aid and the battery's check of
//! surface movement. It builds the Quick Mission a ground target describes,
//! steps the whole world (the live game's tick, no window, no audio) and
//! prints, at each listed time, where every unit that follows a route is.
//! `--kill` destroys a template object (by its ordinal in the template) at a
//! time, to show that a destroyed unit stops where it died. Each unit's lane
//! (feet right of the authored path, for units that share a route) and its
//! legs as driven are printed first. `--surface-only`
//! steps just the surface movement (the world's other half stands still), for
//! runs of hours of mission time.
//!
//! Like `--surface-dump` it adds the ground target templates and unit types
//! the import does not keep yet from the retail media, at runtime.
//! Behaviour: docs/spec/surface-defenses.md, "Movement".
use crate::{AppResult, surface_dump};
use tore_formats::aircraft::AircraftId;
use tore_world::{
    mission::MissionSpec,
    seats::SeatInput,
    surface::{SURFACE_UNIT_BASE, UnitId, movement::Halt},
    world::{Seating, TickOutput, World},
};

pub fn run() -> AppResult<()> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let mut positional = Vec::new();
    let mut seed = 1u32;
    let mut times: Vec<f64> = vec![60.];
    let mut kills: Vec<(u32, f64)> = Vec::new();
    let mut surface_only = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut next = || {
            it.next()
                .ok_or_else(|| format!("{arg} needs a value"))
                .cloned()
        };
        match arg.as_str() {
            "--surface-seed" => seed = next()?.parse()?,
            "--surface-only" => surface_only = true,
            "--at" => {
                times = next()?
                    .split(',')
                    .map(|t| t.parse::<f64>())
                    .collect::<Result<_, _>>()?
            }
            "--kill" => {
                let value = next()?;
                let (ordinal, at) = value.split_once('@').ok_or("--kill ORDINAL@SECONDS")?;
                kills.push((ordinal.parse()?, at.parse()?));
            }
            other if other.starts_with("--") => return Err(format!("unknown {other}").into()),
            other => positional.push(other.to_owned()),
        }
    }
    let [theater, stem] = positional.as_slice() else {
        return Err("--surface-drive THEATER STEM [--surface-seed N] [--at SECONDS,...] [--kill ORDINAL@SECONDS ...] [--surface-only]".into());
    };
    times.sort_by(f64::total_cmp);
    let mut resources = crate::reel::load_assets()?.theater_resources;
    let added = surface_dump::with_retail_surface(&mut resources)?;
    println!("surface-drive: {added} resources added from the retail media");

    let mut spec = MissionSpec::new(theater, AircraftId::F18);
    spec.ground_target = Some(stem.trim_start_matches('~').to_ascii_uppercase());
    spec.surface_seed = seed;
    let mut world = World::new(&spec, &resources, Seating::SinglePlayer)?;
    if let Some(why) = &world.terrain.surface.unresolved {
        return Err(format!("the ground target stands nowhere: {why}").into());
    }
    let units: Vec<(UnitId, String)> = world
        .terrain
        .surface
        .courses
        .keys()
        .map(|id| {
            let unit = world
                .terrain
                .surface
                .unit(*id)
                .expect("a course has a unit");
            (*id, unit.resource.clone())
        })
        .collect();
    for (id, resource) in &units {
        let course = &world.terrain.surface.courses[id];
        println!(
            "surface-drive: unit {:#010x} {resource} route {:.0} ft legs {} speed {:.0} ft/s turn {:.1} deg/s ship {}",
            id.0,
            course.length(),
            course.legs.len(),
            course.legs[0].speed,
            course.turn_rate.to_degrees(),
            u8::from(course.ship),
        );
        let legs: Vec<String> = course
            .legs
            .iter()
            .map(|leg| format!("{:.0},{:.0}", leg.to[0], leg.to[1]))
            .collect();
        println!(
            "surface-drive: unit {:#010x} lane {:.0} legs {}",
            id.0,
            course.lane,
            legs.join(" ")
        );
    }
    println!(
        "surface-drive: units {} moving {}",
        world.terrain.surface.units.len(),
        units.len()
    );

    let mut out = TickOutput::default();
    let mut done = vec![false; kills.len()];
    let mut tick = 0u64;
    for at in times {
        let until = (at * 120.).round() as u64;
        while tick < until {
            let now = tick as f64 / 120.;
            for (n, (ordinal, when)) in kills.iter().enumerate() {
                if !done[n] && now >= *when {
                    done[n] = true;
                    let id = SURFACE_UNIT_BASE + ordinal;
                    if let Some(row) = world
                        .combat
                        .state
                        .targets
                        .iter_mut()
                        .find(|target| target.id == id)
                    {
                        row.hp = 0;
                        println!("surface-drive: t {now:.1} destroyed {id:#010x}");
                    }
                }
            }
            if surface_only {
                world.combat.step_surface(&world.terrain);
            } else {
                let input = SeatInput {
                    tick: world.tick(),
                    ..SeatInput::default()
                };
                world.step(&[input], &mut out)?;
            }
            tick += 1;
        }
        for (id, resource) in &units {
            let Some(mover) = world.combat.surface.unit(*id).and_then(|s| s.mover) else {
                println!(
                    "surface-drive: t {at:.0} unit {:#010x} {resource} standing",
                    id.0
                );
                continue;
            };
            let [x, y, z] = mover.position();
            let [heading, pitch, bank] = mover.attitude();
            let halt = match mover.halt {
                Halt::Moving => "moving",
                Halt::Arrived => "arrived",
                Halt::Destroyed => "destroyed",
            };
            println!(
                "surface-drive: t {at:.0} unit {:#010x} {resource} pos {x:.1} {y:.1} {z:.1} heading {:.1} pitch {:.1} bank {:.1} speed {:.2} leg {} {halt}",
                id.0,
                heading.to_degrees(),
                pitch.to_degrees(),
                bank.to_degrees(),
                mover.speed_feet(),
                mover.leg,
            );
        }
    }
    println!("surface-drive: done at tick {tick}");
    Ok(())
}
