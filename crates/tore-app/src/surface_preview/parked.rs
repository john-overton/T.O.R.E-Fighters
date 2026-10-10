//! Parked aircraft sheets (slice PA1): every aircraft the ground target
//! templates park, gear down at the aircraft convention, and the Clemenceau
//! fleet's aircraft on the carrier deck as the game places them. See
//! docs/spec/surface-defenses.md, "Parked aircraft".
use super::{
    Art, CARRIERS, Canvas, DETAIL, HEADER, Loaded, Media, TILE, VIEWS, View, assemble, bounds,
    carrier_assembly, flight_deck, frame, load, tile,
};
use crate::AppResult;
use std::{collections::BTreeMap, path::Path};
use tore_formats::{
    parked_aircraft::{ParkedType, gear},
    shape::{Shape, object_scale},
};

/// The aircraft the ground target templates park, by exact PT name.
fn parked_types(media: &Media) -> AppResult<Vec<String>> {
    use tore_formats::quick_template::{ObjectKind, Template};
    let mut types = std::collections::BTreeSet::new();
    let names: Vec<String> = media.archives[0]
        .entries
        .keys()
        .filter(|n| n.starts_with("~Q") && n.ends_with(".M"))
        .cloned()
        .collect();
    for name in names {
        let Ok(template) = Template::parse(&name, &media.get(&name)?) else {
            continue;
        };
        for object in &template.objects {
            if let ObjectKind::Named(written) = &object.kind {
                let upper = written.to_ascii_uppercase();
                let pt = if upper.contains('.') {
                    upper
                } else {
                    format!("{upper}.PT")
                };
                if pt.ends_with(".PT") && media.get(&pt).is_ok() {
                    types.insert(pt);
                }
            }
        }
    }
    Ok(types.into_iter().collect())
}

/// A parked type's shape gear up and gear down at the aircraft convention
/// (one third of the scenery scale), with its record and gear word.
fn parked_loads(
    media: &Media,
    art: &mut Art,
    pt: &str,
) -> AppResult<(ParkedType, Option<usize>, Loaded, Loaded)> {
    let record = ParkedType::parse(&media.get(pt)?)?;
    let bytes = media.get(&record.shape)?;
    let word = gear(&bytes)?.word;
    let mut up = load(media, art, &record.shape, Some(&BTreeMap::new()))?;
    let down_state: BTreeMap<usize, i32> = word.map(|w| (w, 1)).into_iter().collect();
    let mut down = load(media, art, &record.shape, Some(&down_state))?;
    up.scale /= 3.;
    down.scale /= 3.;
    up.name = format!("{pt} gear up");
    down.name = format!("{pt} gear down");
    Ok((record, word, up, down))
}

/// The lowest point of a loaded shape, in feet.
fn lowest(loaded: &Loaded) -> f32 {
    bounds(&loaded.shape, loaded.scale).0[2]
}

const LOW: View = View {
    name: "low quarter",
    azimuth: 50.,
    elevation: 6.,
};

/// Every aircraft the templates park, gear down at the aircraft convention:
/// one contact sheet, and a sheet per type with the gear up beside it.
pub(super) fn parked_sheet(out: &Path, media: &Media, art: &mut Art) -> AppResult<()> {
    const COLUMNS: usize = 6;
    const SMALL: [usize; 2] = [420, 300];
    let types = parked_types(media)?;
    let rows = types.len().div_ceil(COLUMNS);
    let mut canvas = Canvas::new(SMALL[0] * COLUMNS, HEADER + SMALL[1] * rows);
    canvas.text(
        &art.font,
        &format!(
            "The {} aircraft types the ground target templates park, gear down, at the aircraft convention (one third of the scenery scale)",
            types.len()
        ),
        10,
        8,
        [255, 255, 255],
    );
    canvas.text(
        &art.font,
        "Gear word: the state word whose branch reaches lowest (tore_formats::parked_aircraft::gear)",
        10,
        32,
        [210, 225, 240],
    );
    for (n, pt) in types.iter().enumerate() {
        let (record, word, up, down) = parked_loads(media, art, pt)?;
        let gear = word.map_or_else(|| "always down".to_owned(), |w| format!("gear {w:#x}"));
        let (center, ppf) = frame(&[&down], &LOW, SMALL);
        let caption = format!("{} {gear},", record.short_name);
        let picture = tile(art, &down, &LOW, center, ppf, &caption, SMALL);
        canvas.blit(
            &picture,
            (n % COLUMNS) * SMALL[0],
            HEADER + (n / COLUMNS) * SMALL[1],
        );
        // Per type: gear up and down from the side, and gear down from low
        // ahead.
        let mut single = Canvas::new(TILE[0] * 3, HEADER + TILE[1]);
        single.text(
            &art.font,
            &format!(
                "{pt}: {} ({}), shape {}, {gear}, class {:#06x}, {} hit points",
                record.short_name, record.name, record.shape, record.class, record.hit_points
            ),
            10,
            8,
            [255, 255, 255],
        );
        let (lo, hi) = bounds(&down.shape, down.scale);
        single.text(
            &art.font,
            &format!(
                "Wheels {:.1} ft below the origin gear down ({:.1} gear up); largest extent {:.0} ft",
                -lowest(&down),
                -lowest(&up),
                (0..3).map(|i| hi[i] - lo[i]).fold(0., f32::max)
            ),
            10,
            32,
            [210, 225, 240],
        );
        let broadside = &VIEWS[1];
        let (center, ppf) = frame(&[&up, &down], broadside, TILE);
        let pictures = [
            tile(art, &up, broadside, center, ppf, "gear up,", TILE),
            tile(art, &down, broadside, center, ppf, "gear down,", TILE),
            {
                let (center, ppf) = frame(&[&down], &LOW, TILE);
                tile(art, &down, &LOW, center, ppf, "gear down,", TILE)
            },
        ];
        for (column, picture) in pictures.iter().enumerate() {
            single.blit(picture, column * TILE[0], HEADER);
        }
        single.png(&out.join(format!("parked-{}.png", pt.trim_end_matches(".PT"))))?;
        println!(
            "Surface preview: parked {pt} {} gear {:?} faces up {} down {}",
            record.shape,
            word,
            up.shape.faces.len(),
            down.shape.faces.len()
        );
    }
    let path = out.join("parked-aircraft.png");
    canvas.png(&path)?;
    println!("Surface preview: {}", path.display());
    Ok(())
}

/// The Clemenceau fleet template's aircraft on the carrier deck as the game
/// places them (`tore_world::surface::parked::deck_spot`): each spot kept in
/// hull units at the deck height, the aircraft gear down at the aircraft
/// convention, the hull at the placed scale with its island and deck parts.
pub(super) fn deck_scene(out: &Path, media: &Media, art: &mut Art) -> AppResult<()> {
    use tore_formats::quick_template::{ObjectKind, Template};
    let template = Template::parse("~QFFLT.M", &media.get("~QFFLT.M")?)?;
    let carrier = &CARRIERS[2];
    let hull_bytes = media.get(carrier.hull)?;
    let deck = flight_deck(&Shape::scenery(&hull_bytes)?).ok_or("CLEM.SH has no deck")?;
    let authored = object_scale(&hull_bytes)?;
    let definition = tore_formats::static_object::Definition::parse(&media.get(carrier.unit)?)?;
    let placed = tore_world::terrain::placed_shape_scale(&definition, &hull_bytes)?;
    let (mut clem, _) = carrier_assembly(media, art, carrier, false)?;
    // The assembly is drawn at the scenery scale; the game places the hull
    // at the placed scale.
    let ratio = (placed / authored) as f32;
    for face in &mut clem.shape.faces {
        for p in &mut face.positions {
            *p = p.map(|v| v * ratio);
        }
    }
    for sprite in &mut clem.shape.billboards {
        sprite.center = sprite.center.map(|v| v * ratio);
        sprite.size = sprite.size.map(|v| v * ratio);
    }
    let ship = template
        .objects
        .iter()
        .find(|o| matches!(&o.kind, ObjectKind::Named(name) if name == "CLEM.NT"))
        .ok_or("no CLEM in ~QFFLT")?;
    let mut parked: Vec<(Loaded, [f32; 3], i16)> = Vec::new();
    let mut off = Vec::new();
    for object in &template.objects {
        let ObjectKind::Named(name) = &object.kind else {
            continue;
        };
        if !name.ends_with(".PT") {
            continue;
        }
        let Some(spot) = tore_world::surface::parked::deck_spot(
            &deck,
            authored,
            placed,
            ship.position,
            ship.angles[0],
            object.position,
        ) else {
            off.push(name.clone());
            continue;
        };
        let (_, _, _, down) = parked_loads(media, art, name)?;
        let lift = spot.feet[1] as f32 - lowest(&down);
        // Template angles are degrees; binary angle units turn the other way.
        let heading = (-(object.angles[0] - ship.angles[0]) as f32 * 65536. / 360.) as i16;
        parked.push((
            down,
            [spot.feet[0] as f32, spot.feet[2] as f32, lift],
            heading,
        ));
    }
    let mut all: Vec<(&Loaded, [f32; 3], i16)> = vec![(&clem, [0.; 3], 0)];
    all.extend(parked.iter().map(|(l, o, h)| (l, *o, *h)));
    let scene = assemble("CLEM.SH with its deck aircraft".into(), &all);
    let caption = format!(
        "{} aircraft on the deck at {:.0} ft ({:.2} ft per hull unit),",
        parked.len(),
        f64::from(deck.height) * placed,
        placed
    );
    for view in [
        &VIEWS[0],
        &VIEWS[2],
        &VIEWS[3],
        &View {
            name: "deck level",
            azimuth: 120.,
            elevation: 4.,
        },
    ] {
        let (center, ppf) = frame(&[&scene], view, DETAIL);
        let picture = tile(art, &scene, view, center, ppf, &caption, DETAIL);
        let name = format!("deck-QFFLT-{}.png", view.name.replace(' ', "-"));
        picture.png(&out.join(name))?;
    }
    // Close looks: each pair of neighbours on the deck, framed on the first
    // and widened to show the deck around it.
    let close = View {
        name: "deck close",
        azimuth: 120.,
        elevation: 14.,
    };
    for (n, (loaded, offset, heading)) in parked.iter().enumerate().step_by(2) {
        let one = assemble("deck aircraft".into(), &[(loaded, *offset, *heading)]);
        let (center, ppf) = frame(&[&one], &close, DETAIL);
        let picture = tile(art, &scene, &close, center, ppf / 3., &caption, DETAIL);
        picture.png(&out.join(format!("deck-QFFLT-close-{n}.png")))?;
    }
    println!(
        "Surface preview: ~QFFLT {} aircraft on the CLEM deck, {} off it {:?}",
        parked.len(),
        off.len(),
        off
    );
    Ok(())
}
