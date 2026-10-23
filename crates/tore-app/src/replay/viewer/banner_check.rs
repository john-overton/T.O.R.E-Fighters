//! CPU-only banner audit and UI composition captures. No renderer or sockets.
use super::*;
use crate::replay::banner::{Banner, Ground, Inspector};

pub(crate) fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let (file, out) = match args.as_slice() {
        [file] => (file, None),
        [file, flag, out] if flag == "--out" => (file, Some(Path::new(out))),
        _ => return Err("--replay-banner-check RECORDING [--out UI_CAPTURE_DIRECTORY]".into()),
    };
    let assets = crate::reel::load_assets()?;
    let mut viewer = Viewer::open(
        Path::new(file),
        &assets.theater_resources,
        &Options::default(),
    )?;
    viewer.finish_tracks();
    let recording = Arc::clone(&viewer.recording);
    let mut inspector = Inspector::default();
    let mut seen = BTreeSet::new();
    let mut samples = 0usize;
    let mut examples = BTreeMap::<String, (u64, Target, Banner)>::new();
    let ground: BTreeMap<_, _> = viewer
        .world
        .airport_scene
        .objects
        .iter()
        .map(|o| {
            let name = recording
                .surface_info(o.id)
                .map(|i| i.label.as_str())
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    [&o.name, &o.object_type]
                        .into_iter()
                        .find(|s| !s.trim().is_empty())
                        .map(String::as_str)
                })
                .map_or_else(|| format!("Ground object {}", o.id), str::to_owned);
            (
                o.id,
                Ground {
                    active: viewer
                        .world
                        .surface
                        .unit(tore_world::surface::UnitId(o.id))
                        .is_some_and(|u| {
                            u.kind == tore_world::surface::UnitKind::Active || u.route.is_some()
                        }),
                    name,
                    position: o.bounds.center,
                },
            )
        })
        .collect();
    let mut previous = None;
    for chunk in 0..recording.chunks().len() {
        let frames = recording.decode_chunk(chunk)?;
        for frame in frames {
            let aircraft = frame.aircraft.iter().map(|a| Target::Aircraft(a.id));
            let weapons = frame
                .projectiles
                .iter()
                .filter(|p| {
                    recording
                        .weapon_info(p.weapon)
                        .is_none_or(|w| w.class != WeaponClass::Gun)
                })
                .map(|p| Target::Missile(p.id));
            let units = frame.surface.iter().map(|s| Target::Ground(s.id));
            let scenery = viewer
                .world
                .airport_scene
                .objects
                .iter()
                .take(if previous.is_none() { ground.len() } else { 0 })
                .map(|o| Target::Ground(o.id));
            for subject in aircraft.chain(weapons).chain(units).chain(scenery) {
                let first = seen.insert(subject);
                if !first && !frame.tick.is_multiple_of(120) {
                    continue;
                }
                let g = if let Target::Ground(id) = subject {
                    ground.get(&id).cloned()
                } else {
                    None
                };
                let banner = inspector
                    .inspect(&recording, &frame, previous.as_ref(), subject, g)
                    .ok_or_else(|| format!("no banner for {subject:?} at {}", frame.tick))?;
                if banner.name.trim().is_empty()
                    || banner.lines().iter().any(|s| {
                        s.split_whitespace()
                            .any(|v| v.parse::<f64>().is_ok_and(|v| !v.is_finite()))
                    })
                {
                    return Err(format!("invalid banner for {subject:?}: {banner:?}").into());
                }
                let class = match subject {
                    Target::Aircraft(id) => {
                        if recording.aircraft_info(id).is_some_and(|a| a.human) {
                            "player"
                        } else {
                            "aircraft"
                        }
                    }
                    Target::Missile(id) => frame
                        .projectiles
                        .iter()
                        .find(|p| p.id == id)
                        .and_then(|p| recording.weapon_info(p.weapon))
                        .map_or("weapon", |w| w.class.name()),
                    _ => "ground",
                };
                examples
                    .entry(class.into())
                    .or_insert((frame.tick, subject, banner));
                samples += 1;
            }
            previous = Some(frame);
        }
    }
    for (class, (tick, subject, banner)) in &examples {
        println!(
            "banner: {class} {subject:?} tick={tick}: {}",
            banner.lines().join(" / ")
        );
    }
    if let Some(out) = out {
        std::fs::create_dir_all(out)?;
        capture(&viewer.ownship, None, "banner-off", out)?;
        for (class, (_, _, banner)) in &examples {
            capture(&viewer.ownship, Some(banner), class, out)?;
        }
    }
    println!(
        "Replay banners: PASS ({} selectable objects, {samples} samples)",
        seen.len()
    );
    Ok(())
}

fn capture(airframe: &Airframe, banner: Option<&Banner>, class: &str, out: &Path) -> AppResult<()> {
    let font = &airframe.font;
    for size in [[3840, 2160], [1920, 1080], [800, 600], [640, 360]] {
        let mut layer = vec![0; overlay::WIDTH * overlay::HEIGHT * 4];
        let mut canvas = FlightCanvas::default();
        let band = crate::flight_ui::message_band(&airframe.hud_font);
        let mut model = Model {
            first: 0,
            last: 1200,
            position: 600.,
            speed: "Paused".into(),
            time: "0:05 / 0:10".into(),
            camera: "External".into(),
            aircraft: "Selected object".into(),
            timer: Some("CPU layout check".into()),
            subtitles: vec!["Recorded subtitle above seven cockpit messages".into()],
            banner: banner.map_or_else(Vec::new, Banner::lines),
            ..Default::default()
        };
        model.subtitle_lift = band.ceil() as i32 + overlay::banner_height(font, &model);
        let messages: Vec<_> = (1..=7).map(|n| format!("Cockpit message {n}")).collect();
        compose(
            &mut canvas,
            size,
            font,
            &mut layer,
            &Interface {
                hidden: false,
                bar: true,
                labels: &[],
                model: &model,
                panels: PanelLayer {
                    pixels: &[],
                    panels: &[],
                    menu: None,
                },
                messages: Some(Messages {
                    lines: &messages,
                    font: &airframe.hud_font,
                    color: [240, 240, 240],
                }),
                compass: None,
            },
        );
        // Solid backing makes the actual alpha-composited UI easy to inspect.
        for p in canvas.pixels.chunks_exact_mut(4) {
            let alpha = u32::from(p[3]);
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * alpha + 64 * (255 - alpha)) / 255) as u8;
            }
            p[3] = 255;
        }
        let name = format!("{class}-{}x{}.png", size[0], size[1]);
        let path = out.join(name);
        if path.exists() {
            return Err(format!("preserving existing image {}", path.display()).into());
        }
        std::fs::write(
            path,
            super::super::png::encode_rgba(size[0], size[1], &canvas.pixels)?,
        )?;
    }
    Ok(())
}
