//! Dumps an NT active-object definition or a Quick Mission ground-target
//! template from a user-owned `FA_2.LIB`, or a one-line summary of every one.
//! Read-only; nothing is written.
//!
//! ```text
//! surface_inspect FA_2.LIB nt SA6.NT
//! surface_inspect FA_2.LIB template QUCOL
//! surface_inspect FA_2.LIB summary
//! ```
use std::path::PathBuf;
use tore_formats::{
    Archive,
    quick_template::{ObjectKind, Template, tables},
    surface_unit::{Ammo, MountKind, SurfaceUnit},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: surface_inspect FA_2.LIB (nt NAME | template STEM | summary)";
    let lib = Archive::open(PathBuf::from(args.first().ok_or(usage)?))?;
    match (args.get(1).map(String::as_str), args.get(2)) {
        (Some("nt"), Some(name)) => {
            let name = name.to_ascii_uppercase();
            let name = if name.contains('.') {
                name
            } else {
                format!("{name}.NT")
            };
            print_unit(&SurfaceUnit::parse(&lib.read(&name)?)?);
        }
        (Some("template"), Some(stem)) => {
            let stem = stem
                .trim_start_matches('~')
                .trim_end_matches(".M")
                .trim_end_matches(".m");
            let stem = stem.to_ascii_uppercase();
            let stem = if lib.entries.contains_key(&format!("~{stem}.M")) {
                stem
            } else {
                format!("Q{stem}")
            };
            let name = format!("~{stem}.M");
            print_template(&Template::parse(&name, &lib.read(&name)?)?);
        }
        (Some("summary"), None) => summary(&lib)?,
        _ => return Err(usage.into()),
    }
    Ok(())
}

fn print_unit(u: &SurfaceUnit) {
    println!("{} ({}, {})", u.resource, u.short_name, u.name);
    println!(
        "  class {:#06x} flags {:#x} callback {}",
        u.class, u.object_flags, u.callback
    );
    println!(
        "  shape {:?} shadow {:?} damaged {:?}",
        u.shape, u.shadow_shape, u.damaged_shape
    );
    println!(
        "  hit points {} signatures {:?} max visible {} year {}",
        u.hit_points, u.signatures, u.max_visible, u.year
    );
    println!(
        "  explosion {} crater {} debris damaged {:?} destroyed {:?}",
        u.explosion, u.crater, u.debris_damaged, u.debris_destroyed
    );
    println!("  movement {:?}", u.movement);
    println!(
        "  npc flags {:#x} script {:?} search {} unready {} attack {} retarget {} zone {}",
        u.npc.flags,
        u.npc.script,
        u.npc.search_frequency,
        u.npc.unready_attack,
        u.npc.attack,
        u.npc.retarget,
        u.npc.zone_dist
    );
    println!(
        "  supply truck: {}, armed: {}",
        u.is_supply_truck(),
        u.armed()
    );
    for (i, m) in u.mounts.iter().enumerate() {
        let ammo = match m.ammo() {
            Ammo::Unlimited => "unlimited".to_owned(),
            Ammo::Rounds(n) => format!("{n} rounds"),
        };
        println!(
            "  mount {i}: {:?} {:?} at {:?} rest {:?} deg arc +-{:?} deg, {ammo}, flags {}",
            m.kind,
            m.store,
            m.position,
            m.slew_degrees(),
            m.limit_degrees(),
            m.flags
        );
    }
}

fn print_template(t: &Template) {
    println!(
        "{} ({} objects, quickpos {:?})",
        t.resource(),
        t.objects.len(),
        t.quickpos
    );
    for o in &t.objects {
        let kind = match &o.kind {
            ObjectKind::Named(n) => n.clone(),
            ObjectKind::Placeholder(p) => format!("<{}>", p.name()),
        };
        println!(
            "  #{:<3} {kind:<12} pos {:?} angle {:?} owner {:?}{} flags {:#x}{} alias {} skill {:?} react {:?} search {:?} start {:?}",
            o.ordinal,
            o.position,
            o.angles[0],
            o.owner.field,
            if o.owner.redfor() { " redfor" } else { " blue" },
            o.flags,
            if o.is_target() { " TARGET" } else { "" },
            o.alias,
            o.skill,
            o.react,
            o.search_dist,
            o.start_time
        );
        if let Some(route) = &o.route {
            println!("      route {:.0} ft:", route.length_feet());
            for w in &route.waypoints {
                println!(
                    "        {} flags {} at {:?} speed {} ft/s wing {:?} react {:?} search {}",
                    w.index, w.flags, w.position, w.speed, w.wing, w.react, w.search_dist
                );
            }
        }
    }
}

fn summary(lib: &Archive) -> Result<(), Box<dyn std::error::Error>> {
    for name in lib.entries.keys().filter(|n| n.ends_with(".NT")) {
        let u = SurfaceUnit::parse(&lib.read(name)?)?;
        let weapons = u
            .mounts
            .iter()
            .filter(|m| m.kind == MountKind::Weapon)
            .count();
        println!(
            "{:<10} class {:#06x} hp {:>5} weapons {} sensor {:?} shape {:?}",
            u.resource, u.class, u.hit_points, weapons, u.sensor, u.shape
        );
    }
    for (theater, list) in tables::TEMPLATES.iter().enumerate() {
        for (entry, stem) in list.iter().enumerate() {
            let name = format!("~{stem}.M");
            let t = Template::parse(&name, &lib.read(&name)?)?;
            println!(
                "{:<7} {entry} {:<9} objects {:>3} targets {:>2} routes {}",
                tables::THEATERS[theater],
                t.stem,
                t.objects.len(),
                t.targets().count(),
                t.routed().count()
            );
        }
    }
    Ok(())
}
