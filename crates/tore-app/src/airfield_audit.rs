//! `--airfield-audit [--svg DIRECTORY] [--track FILE ...] [--field CODE:AIRPORT ...] [THEATER ...]`:
//! the AI's ground routes at every redrawn airport (slice CL-AP,
//! 2026-10-11), walked against the drawn pavement, every runway end
//! reachable over pavement from parking, and every runway object's line
//! (the ILS localizer and glide path) on a drawn runway. Without theaters it
//! audits every layout of the import, the `~` developer layouts included
//! (they hold the lone second tiles). `--svg` writes an overhead picture of
//! each audited airport with its routes, and `--track` adds the ground
//! tracks an AI probe wrote with `--probe-track` (green on the pavement,
//! red off it). `--parked` instead builds every Quick Mission ground target
//! of every base theater and checks that each template aircraft parked on a
//! redrawn airfield stands on its pavement. See
//! docs/formats/redrawn-airports.md ("Audit") and docs/testing/lane-ai.md.
use crate::AppResult;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tore_world::terrain::redrawn::{Built, audit};

/// One probe track: a label and its samples (world `[x, z]`, on pavement).
type Track = (String, Vec<([f64; 2], bool, String)>);

fn read_tracks(paths: &[PathBuf]) -> AppResult<Vec<Track>> {
    let mut tracks: Vec<Track> = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(path)?;
        let mut by_label: BTreeMap<String, (u64, usize)> = BTreeMap::new();
        for line in text.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            let [tick, label, phase, leg, x, z, on, ..] = words[..] else {
                continue;
            };
            let (Ok(tick), Ok(x), Ok(z)) = (tick.parse::<u64>(), x.parse(), z.parse()) else {
                continue;
            };
            // A gap of more than two samples starts a new line.
            let index = match by_label.get(label) {
                Some((last, index)) if tick <= last + 12 => *index,
                _ => {
                    tracks.push((label.to_owned(), Vec::new()));
                    tracks.len() - 1
                }
            };
            by_label.insert(label.to_owned(), (tick, index));
            tracks[index]
                .1
                .push(([x, z], on == "on", format!("{phase}:{leg}")));
        }
    }
    Ok(tracks)
}

pub fn run() -> AppResult<()> {
    let mut args = std::env::args().skip(2);
    let mut svg_dir = None;
    let mut track_paths = Vec::new();
    let mut fields: Vec<(String, u32)> = Vec::new();
    let mut theaters = Vec::new();
    let usage = "--airfield-audit [--svg DIRECTORY] [--track FILE ...] [--field CODE:AIRPORT ...] [--parked] [THEATER ...]";
    let mut parked = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--svg" => svg_dir = Some(PathBuf::from(args.next().ok_or(usage)?)),
            "--track" => track_paths.push(PathBuf::from(args.next().ok_or(usage)?)),
            "--field" => {
                let value = args.next().ok_or(usage)?;
                let (code, airport) = value.split_once(':').ok_or(usage)?;
                fields.push((
                    code.to_ascii_uppercase(),
                    airport.parse().map_err(|_| usage)?,
                ));
            }
            "--parked" => parked = true,
            "--no-audio" => {}
            other if other.starts_with("--") => return Err(usage.into()),
            other => theaters.push(other.to_ascii_uppercase()),
        }
    }
    let resources = crate::reel::load_assets()?.theater_resources;
    if parked {
        return parked_sweep(&resources, &theaters);
    }
    if theaters.is_empty() {
        theaters = if fields.is_empty() {
            tore_formats::theater::map_catalog(&resources)?
                .into_iter()
                .map(|(code, _)| code)
                .collect()
        } else {
            let mut codes: Vec<String> = fields.iter().map(|(c, _)| c.clone()).collect();
            codes.sort();
            codes.dedup();
            codes
        };
    }
    if let Some(dir) = &svg_dir {
        std::fs::create_dir_all(dir)?;
    }
    let tracks = read_tracks(&track_paths)?;
    let (mut objects, mut airfields, mut routes_walked, mut problems) = (0, 0, 0, 0);
    // Pavement on water cells: fields touched, with the retail runway line
    // itself wet (retail placement, kept) or only our added pavement wet.
    let (mut pairs, mut pair_problems) = (0, 0);
    let (mut wet_fields, mut wet_retail, mut wet_ours) = (0, 0, 0);
    for code in &theaters {
        let world = crate::scenery::launch_terrain(&resources, code, None)?;
        let scene = &world.airport_scene;
        // Open water as the terrain mesh draws it: the ground picture's
        // water cutout under a point, else the cell's own water.
        let base = tore_formats::theater::base_theater(&world.layout).unwrap_or("");
        let texel = tore_world::city::mask::ground_texel(&resources, &world.environment, base);
        // Open water as the terrain mesh draws it: the ground picture's
        // water cutout (255) under the point, or the cell's water class or
        // cutout colour. A theater that paints its lakes in ordinary land
        // indices (Vladivostok's Lake Khanka is teal in indices 192 to 197)
        // or at sea-level elevation is not seen: neither marks water.
        let wet_at =
            |x: f64, z: f64| texel(x as i64, z as i64) == Some(255) || world.drawn_as_water(x, z);
        let wanted = |airport: u32| {
            fields.is_empty() || fields.iter().any(|(c, a)| c == code && *a == airport)
        };
        // Runway objects drawn by each pavement owner (by index in redrawn).
        let mut drawn_by: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
        for runway in &scene.runways {
            if !wanted(runway.airport) {
                continue;
            }
            let Some(built) = Built::for_runway(&world.redrawn, runway.object) else {
                continue;
            };
            let Some(owner) = world
                .redrawn
                .iter()
                .position(|b| b.frame == built.frame && !b.patches.is_empty())
            else {
                println!(
                    "airfield-audit: PROBLEM {code} airport {} {}: no pavement",
                    runway.airport, runway.name
                );
                problems += 1;
                continue;
            };
            drawn_by.entry(owner).or_default().push(runway.object);
        }
        for (owner, objects_here) in &drawn_by {
            let pavement = &world.redrawn[*owner];
            airfields += 1;
            let components = audit::components(pavement);
            let here = scene.runway(objects_here[0]).expect("listed above").airport;
            // A pair's continuing runways are one straight runway.
            if let Some(alignment) = audit::alignment(pavement) {
                pairs += 1;
                println!(
                    "airfield-audit: {code} airport {here} pair alignment offset_ft={:.1} heading_deg={:.2} gap_ft={:.1} threshold_gap_ft={:.1}",
                    alignment.offset_ft,
                    alignment.heading_deg,
                    alignment.gap_ft,
                    alignment.threshold_gap_ft
                );
                if let Some(why) = alignment.problem() {
                    println!(
                        "airfield-audit: PROBLEM {code} airport {here}: the pair's runways are not one straight runway: {why}"
                    );
                    problems += 1;
                    pair_problems += 1;
                }
            }
            // Drawn pavement on water cells, and whether the retail runway
            // line is on water there too (then it is retail placement).
            let wet = audit::water(pavement, &wet_at);
            if wet.runway.1 + wet.other.1 > 0 {
                let retail_wet = objects_here.iter().any(|object| {
                    let runway = scene.runway(*object).expect("listed above");
                    let near = runway.threshold(tore_sim::airport::ApproachEnd::Near);
                    let far = runway.threshold(tore_sim::airport::ApproachEnd::Far);
                    (0..=100).any(|k| {
                        let f = f64::from(k) / 100.;
                        wet_at(
                            near[0] + (far[0] - near[0]) * f,
                            near[2] + (far[2] - near[2]) * f,
                        )
                    })
                });
                wet_fields += 1;
                if retail_wet {
                    wet_retail += 1;
                } else {
                    wet_ours += 1;
                }
                println!(
                    "airfield-audit: water {code} airport {here} runway_samples={}/{} other_samples={}/{} retail_runway_line_on_water={retail_wet}",
                    wet.runway.1, wet.runway.0, wet.other.1, wet.other.0
                );
            }
            let mut overlays = Vec::new();
            let mut marks = Vec::new();
            let mut first_airport = 0;
            let mut names = Vec::new();
            for object in objects_here {
                objects += 1;
                let runway = scene.runway(*object).expect("listed above");
                let built = Built::for_runway(&world.redrawn, *object).expect("listed above");
                if first_airport == 0 {
                    first_airport = runway.airport;
                }
                names.push(runway.name.clone());
                let pad = scene.vertical_pad(*object);
                let mut report = format!(
                    "airfield-audit: {code} airport {} {:?} plan={} length_ft={:.0} pad={pad}",
                    runway.airport, runway.name, built.plan, runway.length_ft
                );
                // The ILS: the runway object's line on a drawn runway.
                match audit::line_runway(pavement, runway) {
                    Ok(name) => {
                        let near = pavement
                            .frame
                            .local(runway.threshold(tore_sim::airport::ApproachEnd::Near));
                        let far = pavement
                            .frame
                            .local(runway.threshold(tore_sim::airport::ApproachEnd::Far));
                        report += &format!(
                            " line={} from=({:.0},{:.0}) to=({:.0},{:.0})",
                            name.replace(' ', "_"),
                            near[0],
                            near[1],
                            far[0],
                            far[1]
                        );
                    }
                    Err(why) => {
                        println!(
                            "airfield-audit: PROBLEM {code} airport {} {}: runway line off the drawn runways: {why}",
                            runway.airport, runway.name
                        );
                        problems += 1;
                    }
                }
                let Some(anchors) = built.anchors else {
                    report += " ai_points=none";
                    if !pad {
                        println!(
                            "airfield-audit: PROBLEM {code} airport {} {}: no AI taxi or parking points",
                            runway.airport, runway.name
                        );
                        problems += 1;
                    }
                    println!("{report}");
                    continue;
                };
                let routes = audit::routes(&anchors, runway);
                let mut off_total = 0;
                for route in &routes {
                    routes_walked += 1;
                    let off = audit::walk(&world.redrawn, route);
                    if let Some(first) = off.first() {
                        let local = pavement.frame.local([first[0], 0., first[1]]);
                        println!(
                            "airfield-audit: PROBLEM {code} airport {} {}: {} leaves the pavement at {} samples, first at frame ({:.0}, {:.0})",
                            runway.airport,
                            runway.name,
                            route.name,
                            off.len(),
                            local[0],
                            local[1]
                        );
                        problems += 1;
                        marks.extend(off.iter().map(|p| (*p, "#ff3030")));
                    }
                    off_total += off.len();
                }
                // Every drawn runway end reachable from the parking slots.
                let parking = [anchors.parking[0][0], anchors.parking[0][2]];
                let from = audit::component_at(pavement, &components, parking);
                let mut ends = 0;
                let mut reached = 0;
                for drawn in &pavement.runways {
                    for (end, p) in audit::runway_ends(drawn, 5.).iter().enumerate() {
                        ends += 1;
                        let w = pavement.frame.world(*p);
                        if from.is_some()
                            && audit::component_at(pavement, &components, [w[0], w[2]]) == from
                        {
                            reached += 1;
                        } else {
                            println!(
                                "airfield-audit: PROBLEM {code} airport {} {}: runway {} {} end not reachable over pavement from parking",
                                runway.airport,
                                runway.name,
                                drawn.name,
                                if end == 0 { "near" } else { "far" }
                            );
                            problems += 1;
                        }
                    }
                }
                report += &format!(
                    " routes={} off_samples={off_total} ends_reached={reached}/{ends}",
                    routes.len()
                );
                println!("{report}");
                for (k, route) in routes.iter().enumerate() {
                    // One departure and one arrival route drawn in full,
                    // the slot legs of the others thin.
                    let full = k == 0 || route.name.starts_with("in to slot 1");
                    overlays.push(audit::Overlay {
                        points: route.points.clone(),
                        colour: if route.name.starts_with("out") {
                            "#40a0ff"
                        } else if route.name.starts_with("in") {
                            "#ffd040"
                        } else {
                            "#ffffff"
                        },
                        width: if full { 2.5 } else { 1.0 },
                        dashed: route.name == "takeoff roll",
                    });
                }
                for p in anchors.parking {
                    marks.push(([p[0], p[2]], "#ffffff"));
                }
            }
            if let Some(dir) = &svg_dir {
                let mut track_overlays = Vec::new();
                for (label, samples) in &tracks {
                    let inside: Vec<_> = samples
                        .iter()
                        .filter(|(p, _, _)| pavement.surface.contains_horizontal(p[0], p[1]))
                        .collect();
                    if inside.is_empty() {
                        continue;
                    }
                    track_overlays.push(audit::Overlay {
                        points: inside.iter().map(|(p, _, _)| *p).collect(),
                        colour: "#30ff60",
                        width: 1.5,
                        dashed: false,
                    });
                    // Where a track left the pavement, by phase and leg.
                    let mut runs: Vec<(String, [f64; 2], usize)> = Vec::new();
                    for (p, on, phase) in &inside {
                        if *on {
                            continue;
                        }
                        marks.push((*p, "#ff3030"));
                        match runs.last_mut() {
                            Some((last, _, n)) if last == phase => *n += 1,
                            _ => runs.push((phase.clone(), *p, 1)),
                        }
                    }
                    for (phase, p, n) in runs {
                        let local = pavement.frame.local([p[0], 0., p[1]]);
                        println!(
                            "airfield-audit: track {label} off the pavement in {phase} for {n} samples from frame ({:.0}, {:.0})",
                            local[0], local[1]
                        );
                    }
                }
                overlays.extend(track_overlays);
                let title = format!(
                    "{code} airport {first_airport} {} ({})",
                    names.join(" + "),
                    pavement.plan
                );
                let path = dir.join(format!(
                    "{}-{first_airport}.svg",
                    code.trim_start_matches('~')
                ));
                std::fs::write(&path, audit::svg(pavement, &title, &overlays, &marks))?;
            }
        }
    }
    println!(
        "airfield-audit: {pairs} paired runways checked ({pair_problems} not straight); {wet_fields} airfields have drawn pavement on water cells ({wet_retail} with the retail runway line wet too, {wet_ours} only from our pavement)"
    );
    println!(
        "airfield-audit: {} theaters, {airfields} airfields, {objects} runway objects, {routes_walked} routes, {problems} problems",
        theaters.len()
    );
    if problems > 0 {
        return Err(format!("airfield-audit: {problems} problems").into());
    }
    Ok(())
}

/// `--parked`: every Quick Mission ground target of the base theaters (or
/// the ones named), seed 1, built as a mission builds it; every template
/// aircraft standing inside a redrawn airfield's box must stand on its
/// pavement (slice CL-AP: FX-A found six helicopters on the grass beside
/// dirt strips).
fn parked_sweep(resources: &BTreeMap<String, Vec<u8>>, wanted: &[String]) -> AppResult<()> {
    use tore_formats::quick_template::tables::{ENEMY_NATIONALITY, THEATERS};
    let (mut templates, mut on_fields, mut problems) = (0, 0, 0);
    for (index, theater) in THEATERS.iter().enumerate() {
        if !wanted.is_empty() && !wanted.iter().any(|w| w == theater) {
            continue;
        }
        for stem in tore_world::mission::ground_targets(theater) {
            let target = tore_world::surface::resolve::GroundTarget {
                stem: stem.trim_start_matches('~').to_ascii_uppercase(),
                aaa: 0,
                sam: 0,
                seed: 1,
                enemy_nationality: ENEMY_NATIONALITY[index],
                night_stealth: false,
                variation: tore_world::surface::layout::Variation::ON,
                separation_nm: 20,
            };
            let world = tore_world::terrain::Terrain::for_mission_with(
                resources,
                theater,
                Some(0),
                &tore_world::terrain::Overrides::default(),
                Some(&target),
            )?;
            templates += 1;
            let mut on_field = 0;
            for aircraft in world.surface.parked.iter().filter(|a| a.deck.is_none()) {
                let [x, _, z] = aircraft.position.map(f64::from);
                let Some(field) = world
                    .redrawn
                    .iter()
                    .find(|b| !b.patches.is_empty() && b.surface.contains_horizontal(x, z))
                else {
                    continue;
                };
                on_field += 1;
                if !field.on_pavement(x, z) {
                    let local = field.frame.local([x, 0., z]);
                    println!(
                        "airfield-audit: PROBLEM parked {theater} {} {} off the pavement of {} at frame ({:.0}, {:.0})",
                        target.stem, aircraft.resource, field.plan, local[0], local[1]
                    );
                    problems += 1;
                }
            }
            on_fields += on_field;
        }
    }
    println!(
        "airfield-audit: parked {templates} templates, {on_fields} aircraft on redrawn fields, {problems} problems"
    );
    if problems > 0 {
        return Err(format!("airfield-audit: {problems} parked aircraft off the pavement").into());
    }
    Ok(())
}
