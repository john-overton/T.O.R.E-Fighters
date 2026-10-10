//! `--surface-dump` and `--surface-sheets`: development aids for the surface
//! units (docs/spec/surface-defenses.md). No window opens and nothing is
//! simulated.
//!
//! - `--surface-dump THEATER [STEM] [--defenses AAA SAM] [--surface-seed N]
//!   [--enemy-nationality N] [--night-stealth]` prints the theater's surface
//!   as a mission resolves it, with the ground target template `STEM` when
//!   one is named: every unit with its id, type, side, position and look, the
//!   removed slots, and the digest.
//! - `--surface-dump --all [--seeds N]` resolves every offered template of
//!   every theater at every defense level with seeds 1 to N (3 by default),
//!   builds each theater with every template at heavy defenses, and prints a
//!   line per resolution for the `surface-resolve-all` battery scenario.
//! - `--surface-sheets OUT_DIR [--variants] [THEATER[:STEM] ...]` renders
//!   each template from above, heavy defenses: at its retail spot (no jitter,
//!   no relocation, seed 1), and with `--variants` also placed with seeds 1
//!   and 2. A marker per unit: red Redfor, blue Blue, a ring for targets,
//!   yellow for parked aircraft, green for supply trucks, cyan for battery
//!   radars, orange for battery launchers, magenta for units whose shape the
//!   reader cannot draw yet.
//! - `--surface-dump --sweep [--seeds N] [THEATER ...]` places every
//!   template with seeds 1 to N (20) and reports the site rules each breaks,
//!   and each theater's base-layout batteries; `--surface-dump --starts
//!   [THEATER ...]` builds a mission per theater and reports where Blue and
//!   Red start. The `surface-relocate-sweep` and `surface-start-placement`
//!   scenarios.
//!
//! The import does not keep the templates or the unit types only they name
//! yet, so both commands add what the pack lacks from the retail `FA_2.LIB`
//! the data directory remembers (read at runtime, never stored).
use crate::{
    AppResult,
    camera::Camera,
    reel::{Gpu, HEIGHT, WIDTH},
    scenery::Scenery,
    terrain::{Overrides, Terrain},
};
use std::{collections::BTreeMap, path::Path};
use tore_formats::{
    quick_template::{Placeholder, Template, tables},
    surface_unit::class,
};
use tore_world::surface::{
    IdRange, Origin, Surface, UnitId,
    catalog::Catalog,
    layout::{self, Variation},
    resolve::{self, GroundTarget},
};

/// The pack's theater resources with what the import does not keep yet: the
/// ground target templates and the NT, OT, PT, shape and picture records,
/// read from the retail media: `TORE_GAME_DIR`, else the source the data
/// directory remembers, else the checkout's `gameassets/fighters-anthology`
/// link. Returns how many were added.
fn with_retail_surface(resources: &mut BTreeMap<String, Vec<u8>>) -> AppResult<usize> {
    let data = crate::assets::data_directory()?;
    let candidates = std::env::var_os("TORE_GAME_DIR")
        .map(std::path::PathBuf::from)
        .into_iter()
        .chain(tore_import::media_source::remembered(&data).map(|(path, _)| path))
        .chain([std::path::PathBuf::from("gameassets/fighters-anthology")]);
    let Some(media) = candidates
        .into_iter()
        .find_map(|path| tore_import::MediaSource::detect(&path).ok())
    else {
        return Err(
            "no retail media to read the ground target templates from: set TORE_GAME_DIR".into(),
        );
    };
    let mut added = 0;
    for name in ["FA_2.LIB", "FA_1.LIB"] {
        let Some(archive) = media.optional_archive(name)? else {
            continue;
        };
        for entry in archive.entries.keys() {
            let wanted = (entry.starts_with("~Q") && entry.ends_with(".M"))
                || [".NT", ".OT", ".PT", ".SH", ".PIC"]
                    .iter()
                    .any(|ext| entry.ends_with(ext));
            if wanted && !resources.contains_key(entry) {
                resources.insert(entry.clone(), archive.read(entry)?);
                added += 1;
            }
        }
    }
    Ok(added)
}

fn resources() -> AppResult<BTreeMap<String, Vec<u8>>> {
    let mut resources = crate::reel::load_assets()?.theater_resources;
    let added = with_retail_surface(&mut resources)?;
    println!("surface: {added} resources added from the retail media");
    Ok(resources)
}

/// The base theater index of a layout code (`~UKR1` is Ukraine).
fn theater_index(code: &str) -> AppResult<usize> {
    let base = tore_formats::theater::base_theater(&format!("{code}.MM"))
        .ok_or_else(|| format!("{code}: not a theater"))?;
    tables::THEATERS
        .iter()
        .position(|t| t.eq_ignore_ascii_case(base) || base.starts_with(t))
        .ok_or_else(|| format!("{code}: no ground targets for theater {base}").into())
}

fn overrides() -> Overrides {
    Overrides {
        time: Some([12, 0]),
        wind: None,
        cloud_altitude: Some(0),
    }
}

pub fn run() -> AppResult<()> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let mut positional = Vec::new();
    let mut levels = (3usize, 3usize);
    let mut seed = 1u32;
    let mut nationality = None;
    let mut night = false;
    let mut all = false;
    let mut sweep = false;
    let mut starts = false;
    let mut seeds = None;
    let mut variation = Variation::ON;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut next = || {
            it.next()
                .ok_or_else(|| format!("{arg} needs a value"))
                .cloned()
        };
        match arg.as_str() {
            "--defenses" => levels = (next()?.parse()?, next()?.parse()?),
            "--surface-seed" => seed = next()?.parse()?,
            "--enemy-nationality" => nationality = Some(next()?.parse()?),
            "--night-stealth" => night = true,
            "--all" => all = true,
            "--sweep" => sweep = true,
            "--starts" => starts = true,
            "--no-jitter" => variation.jitter = false,
            "--no-relocate" => variation.relocate = false,
            "--seeds" => seeds = Some(next()?.parse()?),
            other if other.starts_with("--") => return Err(format!("unknown {other}").into()),
            other => positional.push(other.to_owned()),
        }
    }
    let resources = resources()?;
    if all {
        return resolve_all(&resources, seeds.unwrap_or(3));
    }
    if sweep {
        return relocate_sweep(&resources, seeds.unwrap_or(20), &positional);
    }
    if starts {
        return start_placement(&resources, &positional);
    }
    let [theater, rest @ ..] = positional.as_slice() else {
        return Err("--surface-dump THEATER [STEM] [--defenses AAA SAM] [--surface-seed N] [--enemy-nationality N] [--night-stealth] [--no-jitter] [--no-relocate] | --all [--seeds N] | --sweep [--seeds N] [THEATER...] | --starts [THEATER...]".into());
    };
    let index = theater_index(theater)?;
    let target = match rest {
        [] => None,
        [stem] => Some(GroundTarget {
            stem: stem.trim_start_matches('~').to_ascii_uppercase(),
            aaa: levels.0,
            sam: levels.1,
            seed,
            enemy_nationality: nationality.unwrap_or(tables::ENEMY_NATIONALITY[index]),
            night_stealth: night,
            variation,
        }),
        _ => return Err("--surface-dump takes one template".into()),
    };
    let terrain =
        Terrain::for_mission_with(&resources, theater, Some(0), &overrides(), target.as_ref())?;
    print_surface(theater, &terrain.surface);
    Ok(())
}

fn side(surface_side: tore_sim::combat::live::Side) -> &'static str {
    match surface_side.0 {
        1 => "blue",
        2 => "red",
        _ => "neutral",
    }
}

/// The layout's lines: the template's anchor and move, every battery and
/// supply truck, the starts and what could not be added.
fn print_layout(surface: &Surface) {
    if let Some(site) = &surface.template {
        let t = surface.transform;
        println!(
            "surface: layout anchor {} jitter {} relocate {} transform rotation {} translation {} {} pivot {} {} moved-ft {}",
            site.anchor.map_or("none", layout::Anchor::name),
            u8::from(site.settings.variation.jitter),
            u8::from(site.settings.variation.relocate),
            t.rotation_deg,
            t.translation[0],
            t.translation[1],
            t.pivot[0],
            t.pivot[1],
            moved_ft(surface),
        );
    }
    for battery in &surface.batteries {
        let radar = surface.unit(battery.radar);
        println!(
            "surface: battery {:?} side {} radar {:#010x} {} {} launchers {} truck {}",
            battery.system,
            side(battery.side),
            battery.radar.0,
            radar.map_or("?", |u| u.resource.as_str()),
            if battery.radar_added {
                "added"
            } else {
                "adopted"
            },
            battery
                .launchers
                .iter()
                .map(|id| format!("{:#010x}", id.0))
                .collect::<Vec<_>>()
                .join(","),
            battery
                .truck
                .map_or("none".into(), |id| format!("{:#010x}", id.0)),
        );
    }
    for truck in &surface.trucks {
        let unit = surface.unit(truck.id);
        let served = truck.serves.and_then(|id| surface.unit(id));
        let gap = match (unit, served) {
            (Some(u), Some(s)) => {
                let d = [
                    f64::from(u.position[0] - s.position[0]),
                    f64::from(u.position[2] - s.position[2]),
                ];
                format!(" gap-ft {:.0}", d[0].hypot(d[1]))
            }
            _ => String::new(),
        };
        println!(
            "surface: truck {:#010x} {} {} serves {}{gap}",
            truck.id.0,
            unit.map_or("?", |u| u.resource.as_str()),
            if truck.added { "added" } else { "standing" },
            truck
                .serves
                .map_or("any".into(), |id| format!("{:#010x}", id.0)),
        );
    }
    if let Some(starts) = &surface.starts {
        let d = [
            f64::from(starts.blue[0] - starts.target[0]),
            f64::from(starts.blue[1] - starts.target[1]),
        ];
        println!(
            "surface: starts target {} {} blue {} {} heading {} distance-nm {:.1} blue-airfields {} red-airfields {}",
            starts.target[0],
            starts.target[1],
            starts.blue[0],
            starts.blue[1],
            starts.blue_heading_deg,
            d[0].hypot(d[1]) / layout::NM_FT as f64,
            ids(&starts.blue_airfields),
            ids(&starts.red_airfields),
        );
    }
    for note in &surface.layout_notes {
        println!("surface: layout note: {note}");
    }
}

fn ids(list: &[u32]) -> String {
    if list.is_empty() {
        return "none".into();
    }
    list.iter()
        .map(|id| format!("{id:#010x}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// How far the relocation moved the template's targets' centroid, feet.
fn moved_ft(surface: &Surface) -> i64 {
    let t = surface.transform.translation.map(i64::from);
    layout::trig::isqrt((t[0] * t[0] + t[1] * t[1]) as u128) as i64
}

fn print_surface(theater: &str, surface: &Surface) {
    match &surface.template {
        Some(site) => println!(
            "surface: theater {theater} template {} aaa {} sam {} seed {} nationality {} group {} objects {}",
            site.stem,
            site.settings.aaa,
            site.settings.sam,
            site.settings.seed,
            site.settings.enemy_nationality,
            site.group,
            site.objects
        ),
        None => println!("surface: theater {theater} no ground target"),
    }
    for unit in &surface.units {
        let placeholder = match &unit.origin {
            tore_world::surface::Origin::Template {
                placeholder: Some(p),
                ..
            } => format!(" slot <{}>", p.name()),
            Origin::Added => " added".into(),
            _ => String::new(),
        };
        println!(
            "surface: unit {:#010x} {} {:?} class {:#06x} side {} nationality {} pos {} {} {} angle {} target {} skill {} hp {} look {:?} scene {}{placeholder}",
            unit.id.0,
            unit.resource,
            unit.kind,
            unit.class,
            side(unit.side),
            unit.nationality.unwrap_or(-1),
            unit.position[0],
            unit.position[1],
            unit.position[2],
            unit.angles[0],
            u8::from(unit.is_target()),
            unit.skill,
            unit.hit_points,
            unit.look,
            u8::from(unit.in_scene),
        );
    }
    for parked in &surface.parked {
        println!(
            "surface: parked {:#010x} {} side {} pos {} {} {} target {}",
            parked.id.0,
            parked.resource,
            side(parked.side),
            parked.position[0],
            parked.position[1],
            parked.position[2],
            u8::from(parked.target)
        );
    }
    if let Some(site) = &surface.template {
        for ordinal in &site.removed {
            println!(
                "surface: removed {:#010x}",
                UnitId::template(*ordinal).map_or(0, |id| id.0)
            );
        }
        for left in &site.left_out {
            println!(
                "surface: left-out {:#010x} {} ({})",
                UnitId::template(left.ordinal).map_or(0, |id| id.0),
                left.resource,
                left.why
            );
        }
    }
    if let Some(why) = &surface.unresolved {
        println!("surface: unresolved ground target: {why}");
    }
    print_layout(surface);
    for (name, why) in &surface.unreadable {
        println!("surface: unreadable {name}: {why}");
    }
    let count = |range| {
        surface
            .units
            .iter()
            .filter(|u| u.id.range() == range)
            .count()
    };
    println!(
        "surface: totals units {} layout {} template {} parked {} trucks {} added-trucks {} batteries {} added-radars {} targets {} not-drawn {} unreadable {}",
        surface.units.len(),
        count(IdRange::Layout),
        count(IdRange::Template),
        surface.parked.len(),
        surface.trucks.len(),
        count(IdRange::SupplyTruck),
        surface.batteries.len(),
        count(IdRange::BatteryRadar),
        surface.targets().count(),
        surface.units.iter().filter(|u| !u.in_scene).count(),
        surface.unreadable.len()
    );
    println!("surface: digest {:#018x}", surface.digest());
}

/// `--surface-dump --all`: see the module comment.
fn resolve_all(resources: &BTreeMap<String, Vec<u8>>, seeds: u32) -> AppResult<()> {
    let mut errors = 0usize;
    let mut resolutions = 0usize;
    let mut templates = 0usize;
    for (index, theater) in tables::THEATERS.iter().enumerate() {
        // The base layout's air defenses, by side.
        let base = Terrain::for_mission(resources, theater, Some(0), &overrides())?;
        let defenses = |side_id: u32| {
            base.surface
                .units
                .iter()
                .filter(|u| u.side.0 == side_id && u.class & (class::SAM | class::AAA) != 0)
                .count()
        };
        println!(
            "surface-base: {theater} units {} sam-aaa-red {} sam-aaa-blue {} trucks {} not-drawn {} digest {:#018x}",
            base.surface.units.len(),
            defenses(2),
            defenses(1),
            base.surface.trucks.len(),
            base.surface.units.iter().filter(|u| !u.in_scene).count(),
            base.surface.digest()
        );
        let nationality = tables::ENEMY_NATIONALITY[index];
        for stem in tables::TEMPLATES[index] {
            templates += 1;
            let name = format!("~{stem}.M");
            let template = match resources
                .get(&name)
                .ok_or_else(|| format!("missing {name}"))
                .and_then(|bytes| Template::parse(&name, bytes).map_err(|e| e.to_string()))
            {
                Ok(template) => template,
                Err(error) => {
                    errors += 1;
                    println!("surface-all: error {theater} {stem}: {error}");
                    continue;
                }
            };
            let slots = |which: Placeholder| {
                template
                    .objects
                    .iter()
                    .filter(|o| o.placeholder() == Some(which))
                    .count()
            };
            let (sam_slots, aaa_slots) = (slots(Placeholder::Sam), slots(Placeholder::Aaa));
            for level in 0..4 {
                for seed in 1..=seeds {
                    resolutions += 1;
                    let target = GroundTarget {
                        stem: (*stem).to_owned(),
                        aaa: level,
                        sam: level,
                        seed,
                        enemy_nationality: nationality,
                        night_stealth: false,
                        variation: Variation::ON,
                    };
                    let mut catalog = Catalog::new(resources);
                    let resolved = match resolve::template(
                        &template,
                        &target,
                        &mut catalog,
                        Some(base.environment.map.as_str()),
                    )
                    .and_then(|t| resolve::surface(Default::default(), Some(t)))
                    {
                        Ok(surface) => surface,
                        Err(error) => {
                            errors += 1;
                            println!(
                                "surface-all: error {theater} {stem} level {level} seed {seed}: {error}"
                            );
                            continue;
                        }
                    };
                    let site = resolved.template.as_ref().expect("a template site");
                    let manned = resolved
                        .units
                        .iter()
                        .filter(|u| {
                            matches!(&u.origin, tore_world::surface::Origin::Template { placeholder: Some(p), .. } if p.is_defense())
                        })
                        .count();
                    println!(
                        "surface-all: {theater} {stem} level {level} seed {seed} objects {} sam-slots {sam_slots} aaa-slots {aaa_slots} targets {} manned {manned} removed {} units {} parked {} left-out {} digest {:#018x}",
                        site.objects,
                        resolved.targets().count(),
                        site.removed.len(),
                        resolved.units.len(),
                        resolved.parked.len(),
                        site.left_out.len(),
                        resolved.digest()
                    );
                }
            }
            // The whole scene with the template at heavy defenses: every type
            // reads, and its units join the airport scene.
            let target = GroundTarget {
                stem: (*stem).to_owned(),
                aaa: 3,
                sam: 3,
                seed: 1,
                enemy_nationality: nationality,
                night_stealth: false,
                variation: Variation::ON,
            };
            match Terrain::for_mission_with(
                resources,
                theater,
                Some(0),
                &overrides(),
                Some(&target),
            ) {
                Ok(built) => {
                    let surface = &built.surface;
                    let not_drawn: Vec<&str> = surface
                        .template_units()
                        .filter(|u| !u.in_scene)
                        .map(|u| u.resource.as_str())
                        .collect();
                    println!(
                        "surface-scene: {theater} {stem} units {} drawn {} not-drawn {} {}",
                        surface.template_units().count(),
                        surface.template_units().filter(|u| u.in_scene).count(),
                        not_drawn.len(),
                        summary(&not_drawn)
                    );
                }
                Err(error) => {
                    errors += 1;
                    println!("surface-all: error {theater} {stem} scene: {error}");
                }
            }
        }
    }
    println!("surface-all: {resolutions} resolutions, {templates} templates, {errors} errors");
    if errors > 0 {
        return Err(format!("{errors} surface resolutions failed").into());
    }
    Ok(())
}

/// The stems `--sweep` and `--starts` cover: every offered template but the
/// "nothing" ones, of the named theaters or of all.
fn jobs(wanted: &[String]) -> AppResult<Vec<(usize, &'static str, Vec<&'static str>)>> {
    let mut out = Vec::new();
    for (index, theater) in tables::THEATERS.iter().enumerate() {
        if !wanted.is_empty() && !wanted.iter().any(|w| w.eq_ignore_ascii_case(theater)) {
            continue;
        }
        let stems = tables::TEMPLATES[index]
            .iter()
            .copied()
            .filter(|stem| !stem.ends_with("NOTH"))
            .collect();
        out.push((index, *theater, stems));
    }
    if out.is_empty() {
        return Err(format!("no theater among {wanted:?}").into());
    }
    Ok(out)
}

fn heavy(stem: &str, index: usize, seed: u32) -> GroundTarget {
    GroundTarget {
        stem: stem.to_owned(),
        aaa: 3,
        sam: 3,
        seed,
        enemy_nationality: tables::ENEMY_NATIONALITY[index],
        night_stealth: false,
        variation: Variation::ON,
    }
}

/// `--surface-dump --sweep [--seeds N] [THEATER...]`: every offered
/// template of every (named) theater placed at heavy defenses with seeds 1
/// to N (20 by default): its anchor and move, the site rules it breaks
/// ([`Terrain::audit_surface`]'s), its batteries and trucks against their
/// rules, and its digest; each theater's base-layout batteries. For the
/// `surface-relocate-sweep` scenario.
fn relocate_sweep(
    resources: &BTreeMap<String, Vec<u8>>,
    seeds: u32,
    wanted: &[String],
) -> AppResult<()> {
    let (mut placements, mut relocated, mut problems, mut errors) =
        (0usize, 0usize, 0usize, 0usize);
    for (index, theater, stems) in jobs(wanted)? {
        let terrain = Terrain::for_mission(resources, theater, Some(0), &overrides())?;
        let base = &terrain.surface;
        let count = |system| base.batteries.iter().filter(|b| b.system == system).count();
        use tore_world::surface::BatterySystem as S;
        let in_battery: usize = base.batteries.iter().map(|b| b.launchers.len()).sum();
        let launchers = base
            .units
            .iter()
            .filter(|u| S::of_launcher(&u.resource).is_some() && u.side.0 != 0)
            .count();
        println!(
            "surface-base-batteries: {theater} batteries {} sa2 {} sa3 {} sa6 {} hawk {} adopted {} added {} launchers {launchers} in-batteries {in_battery} notes {} digest {:#018x}",
            base.batteries.len(),
            count(S::Sa2),
            count(S::Sa3),
            count(S::Sa6),
            count(S::Hawk),
            base.batteries.iter().filter(|b| !b.radar_added).count(),
            base.batteries.iter().filter(|b| b.radar_added).count(),
            base.layout_notes.len(),
            base.digest(),
        );
        let site = tore_world::terrain::SurfaceSite::load(resources, theater)?;
        for stem in stems {
            for seed in 1..=seeds {
                placements += 1;
                let surface = match site.place(
                    resources,
                    &terrain.theater,
                    Some(&heavy(stem, index, seed)),
                ) {
                    Ok(surface) => surface,
                    Err(error) => {
                        errors += 1;
                        println!("surface-sweep: error {theater} {stem} seed {seed}: {error}");
                        continue;
                    }
                };
                let mut broken = site.audit(resources, &terrain.theater, &surface);
                broken.extend(rule_problems(&surface));
                let anchor = surface.template.as_ref().and_then(|t| t.anchor);
                if anchor.is_some() && !surface.transform.is_identity() {
                    broken.push("an anchored template moved".into());
                }
                if !surface.transform.is_identity() {
                    relocated += 1;
                }
                problems += broken.len();
                for problem in &broken {
                    println!("surface-sweep-problem: {theater} {stem} seed {seed}: {problem}");
                }
                println!(
                    "surface-sweep: {theater} {stem} seed {seed} anchor {} moved-ft {} rotation {} units {} trucks {} batteries {} problems {} digest {:#018x}",
                    anchor.map_or("none", layout::Anchor::name),
                    moved_ft(&surface),
                    surface.transform.rotation_deg,
                    surface.template_units().count(),
                    surface
                        .units
                        .iter()
                        .filter(|u| u.id.range() == IdRange::SupplyTruck)
                        .count(),
                    surface.batteries.len(),
                    broken.len(),
                    surface.digest(),
                );
            }
        }
    }
    println!(
        "surface-sweep: {placements} placements, {relocated} relocated, {problems} problems, {errors} errors"
    );
    if errors > 0 {
        return Err(format!("{errors} placements failed").into());
    }
    Ok(())
}

/// The battery and truck rules a placed surface breaks: battery sizes
/// against their caps, one added truck per manned slot and per template
/// battery, each 200 to 400 ft from the unit it serves.
fn rule_problems(surface: &Surface) -> Vec<String> {
    let mut out = Vec::new();
    for battery in &surface.batteries {
        if battery.launchers.len() > battery.system.cap() {
            out.push(format!(
                "{:?} battery of {:#010x} has {} launchers",
                battery.system,
                battery.launchers[0].0,
                battery.launchers.len()
            ));
        }
        if battery.launchers[0].range() == IdRange::Template && battery.truck.is_none() {
            out.push(format!(
                "battery of {:#010x} has no truck",
                battery.launchers[0].0
            ));
        }
    }
    let slots = surface
        .template_units()
        .filter(|u| {
            matches!(&u.origin, Origin::Template { placeholder: Some(p), .. } if p.is_defense())
        })
        .count();
    let template_batteries = surface
        .batteries
        .iter()
        .filter(|b| b.launchers[0].range() == IdRange::Template)
        .count();
    let added = surface.trucks.iter().filter(|t| t.added).count();
    if added != slots + template_batteries && surface.layout_notes.is_empty() {
        out.push(format!(
            "{added} added trucks for {slots} manned slots and {template_batteries} batteries"
        ));
    }
    for truck in surface.trucks.iter().filter(|t| t.added) {
        let (Some(unit), Some(served)) = (
            surface.unit(truck.id),
            truck.serves.and_then(|id| surface.unit(id)),
        ) else {
            continue;
        };
        let d2 = (i64::from(unit.position[0] - served.position[0])).pow(2)
            + (i64::from(unit.position[2] - served.position[2])).pow(2);
        let [lo, hi] = layout::TRUCK_FT;
        if d2 < (lo - 1).pow(2) || d2 > (hi + 1).pow(2) {
            out.push(format!(
                "truck {:#010x} stands {d2} sq ft from {:#010x}",
                truck.id.0, served.id.0
            ));
        }
    }
    out
}

/// `--surface-dump --starts [THEATER...]`: for one template per theater
/// (the first that relocates with seed 1, else the first), builds the
/// mission with a ground target, an airborne start at 20,000 ft and Red 20
/// nm ahead, and prints where Blue and Red start: Blue's distance from the
/// target, its bearing off the line toward its own side, its heading off the
/// target, Red's distance from Blue, and whether both are on the map. For
/// the `surface-start-placement` scenario.
fn start_placement(resources: &BTreeMap<String, Vec<u8>>, wanted: &[String]) -> AppResult<()> {
    use tore_world::{
        mission::{Defense, MissionSpec, Skill, Start},
        world::{Seating, World},
    };
    let mut checked = 0;
    for (index, theater, stems) in jobs(wanted)? {
        let terrain = Terrain::for_mission(resources, theater, Some(0), &overrides())?;
        let site = tore_world::terrain::SurfaceSite::load(resources, theater)?;
        let stem = stems
            .iter()
            .copied()
            .find(|stem| {
                site.place(resources, &terrain.theater, Some(&heavy(stem, index, 1)))
                    .is_ok_and(|s| !s.transform.is_identity())
            })
            .unwrap_or(stems[0]);
        let mut spec = MissionSpec::new(theater, tore_formats::aircraft::AircraftId::F18);
        spec.ground_target = Some(stem.to_owned());
        spec.aaa = Defense::Heavy;
        spec.sam = Defense::Heavy;
        spec.surface_seed = 1;
        spec.enemy_nationality = tables::ENEMY_NATIONALITY[index] as u8;
        spec.wings[3].count = 1;
        spec.wings[3].skill = Skill::Average;
        spec.separation_nm = 20;
        spec.start = Start::Airborne {
            altitude_ft: 20_000,
        };
        let world = World::new(&spec, resources, Seating::SinglePlayer)?;
        let Some(starts) = world.terrain.surface.starts.clone() else {
            println!("surface-start: {theater} {stem} no starts");
            continue;
        };
        let nm = layout::NM_FT as f64;
        let target = starts.target.map(f64::from);
        let blue = world.cockpits[0].flight.position;
        let yaw = world.cockpits[0].flight.yaw;
        let to_blue = [blue[0] - target[0], blue[2] - target[1]];
        let distance = to_blue[0].hypot(to_blue[1]) / nm;
        let angle = |v: [f64; 2]| v[0].atan2(v[1]).to_degrees();
        let off = |a: f64, b: f64| ((a - b + 540.).rem_euclid(360.) - 180.).abs();
        let side_off = site.front().map_or(0., |front| {
            let toward = [
                (front.blue[0] - front.red[0]) as f64,
                (front.blue[1] - front.red[1]) as f64,
            ];
            off(angle(to_blue), angle(toward))
        });
        let heading_off = off(yaw.to_degrees(), angle([-to_blue[0], -to_blue[1]]));
        let red = world
            .combat
            .state
            .targets
            .iter()
            .filter(|t| t.side == tore_world::ai_wings::ENEMY_SIDE && t.aircraft.is_some())
            .map(|t| t.position)
            .next();
        let extent = [
            (terrain.theater.cols as f64 - 1.) * f64::from(tore_formats::theater::CELL_FEET),
            (terrain.theater.rows as f64 - 1.) * f64::from(tore_formats::theater::CELL_FEET),
        ];
        let on_map =
            |p: [f64; 3]| (0. ..=extent[0]).contains(&p[0]) && (0. ..=extent[1]).contains(&p[2]);
        let red_nm = red.map_or(-1., |r| (r[0] - blue[0]).hypot(r[2] - blue[2]) / nm);
        println!(
            "surface-start: {theater} {stem} blue-nm {distance:.2} side-off-deg {side_off:.1} heading-off-deg {heading_off:.1} red-nm {red_nm:.2} blue-on-map {} red-on-map {} front {}",
            u8::from(on_map(blue)),
            u8::from(red.is_some_and(on_map)),
            u8::from(site.front().is_some()),
        );
        checked += 1;
    }
    println!("surface-start: {checked} theaters");
    Ok(())
}

/// `T72.NT x3, SA3.NT x9`.
fn summary(names: &[&str]) -> String {
    let mut counts = BTreeMap::<&str, usize>::new();
    for name in names {
        *counts.entry(name).or_default() += 1;
    }
    counts
        .iter()
        .map(|(name, n)| format!("{name} x{n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `--surface-sheets`: see the module comment.
pub fn sheets() -> AppResult<()> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let Some((out, wanted)) = args.split_first() else {
        return Err("--surface-sheets OUTPUT_DIRECTORY [--variants] [THEATER[:STEM] ...]".into());
    };
    // `--variants`: the retail spot (no jitter, no relocation) and two
    // seeds with both on; otherwise the retail spot alone.
    let all_variants = wanted.iter().any(|w| w == "--variants");
    let wanted: Vec<String> = wanted
        .iter()
        .filter(|w| *w != "--variants")
        .cloned()
        .collect();
    let wanted = wanted.as_slice();
    let variants: &[(&str, Variation, u32)] = if all_variants {
        &[
            ("retail", Variation::OFF, 1),
            ("seed1", Variation::ON, 1),
            ("seed2", Variation::ON, 2),
        ]
    } else {
        &[("retail", Variation::OFF, 1)]
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let resources = resources()?;
    let mut jobs: Vec<(String, Vec<String>)> = Vec::new();
    if wanted.is_empty() {
        for (index, theater) in tables::THEATERS.iter().enumerate() {
            jobs.push((
                (*theater).to_owned(),
                tables::TEMPLATES[index]
                    .iter()
                    .filter(|stem| !stem.ends_with("NOTH"))
                    .map(|s| (*s).to_owned())
                    .collect(),
            ));
        }
    } else {
        for item in wanted {
            let (theater, stem) = item.split_once(':').unwrap_or((item, ""));
            let index = theater_index(theater)?;
            let stems = if stem.is_empty() {
                tables::TEMPLATES[index]
                    .iter()
                    .filter(|stem| !stem.ends_with("NOTH"))
                    .map(|s| (*s).to_owned())
                    .collect()
            } else {
                vec![stem.to_ascii_uppercase()]
            };
            jobs.push((theater.to_ascii_uppercase(), stems));
        }
    }
    let mut gpu: Option<Gpu> = None;
    for (theater, stems) in jobs {
        let index = theater_index(&theater)?;
        for (stem, (variant, variation, seed)) in stems
            .iter()
            .flat_map(|stem| variants.iter().map(move |variant| (stem.clone(), *variant)))
        {
            let target = GroundTarget {
                stem: stem.clone(),
                aaa: 3,
                sam: 3,
                seed,
                enemy_nationality: tables::ENEMY_NATIONALITY[index],
                night_stealth: false,
                variation,
            };
            let world = Terrain::for_mission_with(
                &resources,
                &theater,
                Some(0),
                &overrides(),
                Some(&target),
            )?;
            let mut scenery = Scenery::build(&resources, &world)?;
            let gpu = match &mut gpu {
                Some(gpu) => {
                    // Another theater's land and sky on the same device.
                    gpu.sim = crate::sim_renderer::SimRenderer::new(
                        &gpu.device,
                        &gpu.queue,
                        wgpu::TextureFormat::Rgba8UnormSrgb,
                        &scenery,
                        crate::graphics::Options::default(),
                        4,
                    );
                    gpu
                }
                None => gpu.insert(pollster::block_on(Gpu::new(&scenery))?),
            };
            let surface = &world.surface;
            let points: Vec<([f64; 3], Marker)> = marks(&world, surface);
            if points.is_empty() {
                continue;
            }
            for (view, pitch, scale) in [("top", -70.0, 1.0), ("close", -38.0, 0.45)] {
                let camera = overhead(&points, pitch, scale);
                scenery.resolve_palette(&world, camera.position[1]);
                scenery.set_origin(camera.position);
                let hidden = std::collections::BTreeSet::new();
                let geometry = std::sync::Arc::clone(scenery.static_geometry_where(&hidden));
                gpu.sim.airports(&gpu.device, &gpu.queue, &geometry);
                let mut pixels = gpu.pixels(&camera, &world, &scenery)?;
                for (point, marker) in &points {
                    if let Some(at) = camera.project([WIDTH, HEIGHT], *point) {
                        marker.draw(&mut pixels, at, view == "top");
                    }
                }
                let path = out.join(format!("{theater}-{stem}-{variant}-{view}.png"));
                std::fs::write(
                    &path,
                    crate::replay::png::encode_rgba(WIDTH, HEIGHT, &pixels)?,
                )?;
                println!(
                    "surface sheet: {} ({} units, {} parked, {} not drawn, digest {:#018x})",
                    path.display(),
                    surface.template_units().count(),
                    surface.parked.len(),
                    surface.template_units().filter(|u| !u.in_scene).count(),
                    surface.digest()
                );
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Marker {
    color: [u8; 3],
    ring: bool,
}

impl Marker {
    /// Draws the marker at pixel `at`; a `full` dot, or a small one beside
    /// the unit in a close view so the shape stays visible.
    fn draw(&self, pixels: &mut [u8], at: [f64; 2], full: bool) {
        let [mut cx, mut cy] = at.map(|v| v.round() as i32);
        if !full {
            cx += 14;
            cy -= 14;
        }
        let radius: i32 = match (self.ring, full) {
            (true, true) => 9,
            (true, false) => 6,
            (false, true) => 4,
            (false, false) => 3,
        };
        let inner = if full { 49 } else { 16 };
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let d2 = dx * dx + dy * dy;
                let inside = if self.ring {
                    (inner..=radius * radius).contains(&d2)
                } else {
                    d2 <= radius * radius
                };
                let (x, y) = (cx + dx, cy + dy);
                if !inside || x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
                    continue;
                }
                let at = ((y as u32 * WIDTH + x as u32) * 4) as usize;
                pixels[at..at + 3].copy_from_slice(&self.color);
                pixels[at + 3] = 255;
            }
        }
    }
}

/// A marker per template unit and parked aircraft, at its ground point.
fn marks(world: &Terrain, surface: &Surface) -> Vec<([f64; 3], Marker)> {
    let ground = |p: [i32; 3]| {
        let (x, z) = (f64::from(p[0]), f64::from(p[2]));
        [x, f64::from(world.height(x as f32, z as f32)) + 20., z]
    };
    let mut out = Vec::new();
    let radars: std::collections::BTreeSet<UnitId> =
        surface.batteries.iter().map(|b| b.radar).collect();
    let launchers: std::collections::BTreeSet<UnitId> = surface
        .batteries
        .iter()
        .flat_map(|b| b.launchers.iter().copied())
        .collect();
    let template_or_added = surface
        .units
        .iter()
        .filter(|u| u.id.range() != IdRange::Layout || radars.contains(&u.id));
    for unit in template_or_added {
        let color = if !unit.in_scene {
            [255, 0, 255]
        } else if radars.contains(&unit.id) {
            [0, 230, 230]
        } else if launchers.contains(&unit.id) {
            [255, 150, 0]
        } else if unit.supply_truck {
            [40, 220, 60]
        } else if unit.side.0 == 1 {
            [40, 120, 255]
        } else {
            [235, 40, 40]
        };
        out.push((ground(unit.position), Marker { color, ring: false }));
        if unit.is_target() {
            out.push((
                ground(unit.position),
                Marker {
                    color: [255, 255, 255],
                    ring: true,
                },
            ));
        }
    }
    for parked in &surface.parked {
        out.push((
            ground(parked.position),
            Marker {
                color: [255, 220, 0],
                ring: parked.target,
            },
        ));
    }
    out
}

/// A camera over the units at `pitch_deg` (down), looking north, far
/// enough back to frame all of them, then brought in by `scale`.
fn overhead(points: &[([f64; 3], Marker)], pitch_deg: f64, scale: f64) -> Camera {
    let (mut min, mut max) = ([f64::MAX; 2], [f64::MIN; 2]);
    let mut top = f64::MIN;
    for (p, _) in points {
        min = [min[0].min(p[0]), min[1].min(p[2])];
        max = [max[0].max(p[0]), max[1].max(p[2])];
        top = top.max(p[1]);
    }
    let center = [(min[0] + max[0]) / 2., (min[1] + max[1]) / 2.];
    let span = (max[0] - min[0])
        .max((max[1] - min[1]) * 16. / 9.)
        .max(2000.);
    let pitch = pitch_deg.to_radians();
    // The view is 60 degrees high, so tan(half width) is (16 / 9) / sqrt(3).
    let half_width = 16. / 9. / 3f64.sqrt();
    let distance = (span * 1.15 / 2. + 500.) / half_width * scale;
    let mut camera = Camera::new();
    camera.yaw = 0.;
    camera.pitch = pitch as f32;
    camera.position = [
        center[0],
        top + distance * -pitch.sin(),
        center[1] - distance * pitch.cos(),
    ];
    camera
}
