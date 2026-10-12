//! `--cockpit-sheet OUT_DIR`: offscreen renders of every flyable aircraft's
//! cockpit as flight draws it (forward view, mirrors live, no HUD), the
//! cockpit art with the mirror masks tinted, and two contact sheets. A
//! development aid for the mirror review (docs/baselines/mirrors.md): no
//! window opens, nothing is simulated beyond the starting pose.
//!
//! Each tile carries a label strip in the game's own font: the aircraft, its
//! cockpit art family, and which mirror regions are fitted (`C` centre, `L`
//! and `R` side, the reviewed flat-fill floods of `mirrors.rs`).
use crate::{
    AppResult,
    attitude::Basis,
    camera::Camera,
    reel::{Gpu, HEIGHT, WIDTH},
    render_snapshot::CombatArt,
    scenery::Scenery,
    snapshot::RenderSnapshot,
    terrain::{Overrides, Terrain},
};
use std::path::Path;
use tore_formats::aircraft::AircraftId;

const LABEL: usize = 26;
const SHEET_COLUMNS: usize = 6;
const TILE_WIDTH: usize = 480;

struct Tile {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl Tile {
    fn from_rgba(width: usize, height: usize, rgba: Vec<u8>) -> Self {
        Self {
            width,
            height,
            rgba,
        }
    }
    /// A box-filtered copy `width` pixels wide.
    fn scaled(&self, width: usize) -> Self {
        let height = (self.height * width).div_ceil(self.width);
        let mut rgba = vec![0u8; width * height * 4];
        for y in 0..height {
            for x in 0..width {
                let (x0, x1) = (
                    x * self.width / width,
                    ((x + 1) * self.width).div_ceil(width),
                );
                let (y0, y1) = (
                    y * self.height / height,
                    ((y + 1) * self.height).div_ceil(height),
                );
                let mut sum = [0u32; 4];
                let mut n = 0;
                for yy in y0..y1.min(self.height) {
                    for xx in x0..x1.min(self.width) {
                        for (c, total) in sum.iter_mut().enumerate() {
                            *total += u32::from(self.rgba[(yy * self.width + xx) * 4 + c]);
                        }
                        n += 1;
                    }
                }
                for (c, total) in sum.iter().enumerate() {
                    rgba[(y * width + x) * 4 + c] = (total / n.max(1)) as u8;
                }
            }
        }
        Self::from_rgba(width, height, rgba)
    }
    /// The tile under a dark strip of `text` in the game's font.
    fn labelled(&self, font: &tore_formats::font::Font, text: &str, scale: usize) -> Self {
        let strip = LABEL * scale;
        let mut rgba = vec![0u8; self.width * (self.height + strip) * 4];
        for px in rgba.chunks_exact_mut(4).take(self.width * strip) {
            px.copy_from_slice(&[16, 16, 24, 255]);
        }
        rgba[self.width * strip * 4..].copy_from_slice(&self.rgba);
        let mut x = 6 * scale;
        for ch in text.chars() {
            let code = ch as usize;
            let Some(glyph) = font.glyphs.get(code) else {
                continue;
            };
            for &(gx, gy) in &glyph.pixels {
                for dy in 0..scale {
                    for dx in 0..scale {
                        let (px, py) = (x + gx * scale + dx, 4 * scale + gy * scale + dy);
                        if px < self.width && py < strip {
                            let at = (py * self.width + px) * 4;
                            rgba[at..at + 4].copy_from_slice(&[255, 240, 160, 255]);
                        }
                    }
                }
            }
            x += glyph.advance * scale;
        }
        Self::from_rgba(self.width, self.height + strip, rgba)
    }
    fn write(&self, path: &Path) -> AppResult<()> {
        std::fs::write(
            path,
            crate::replay::png::encode_rgba(self.width as u32, self.height as u32, &self.rgba)?,
        )?;
        Ok(())
    }
}

/// The cockpit art over mid grey with each mirror region tinted by its seed
/// index (centre red, left green, right blue).
fn mask_tile(source: &crate::menu::Sprite, masks: Option<&crate::mirrors::Masks>) -> Tile {
    let (w, h) = (source.width, source.height);
    let mut rgba = vec![0u8; w * h * 4];
    for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
        let s = &source.rgba[i * 4..i * 4 + 4];
        let a = f32::from(s[3]) / 255.;
        for c in 0..3 {
            px[c] = (f32::from(s[c]) * a + 90. * (1. - a)) as u8;
        }
        px[3] = 255;
        if let Some(m) = masks {
            let tint = match m.pixels[i] {
                1 => [255u8, 40, 40],
                2 => [40, 255, 40],
                3 => [60, 120, 255],
                _ => continue,
            };
            for c in 0..3 {
                px[c] = ((u16::from(px[c]) + u16::from(tint[c])) / 2) as u8;
            }
        }
    }
    Tile::from_rgba(w, h, rgba)
}

fn describe(masks: Option<&crate::mirrors::Masks>) -> String {
    let Some(m) = masks else {
        return "NO MIRRORS FITTED".into();
    };
    let mut out = String::from("FITTED ");
    for (rect, name) in m.rects.iter().zip(["C", "L", "R"]) {
        if rect[2] > 0. && rect[3] > 0. {
            out.push_str(name);
        }
    }
    out
}

fn sheet(tiles: &[Tile], path: &Path) -> AppResult<()> {
    sheet_columns(tiles, SHEET_COLUMNS, path)
}
fn sheet_columns(tiles: &[Tile], columns: usize, path: &Path) -> AppResult<()> {
    let rows = tiles.len().div_ceil(columns);
    let cell_height = tiles.iter().map(|t| t.height).max().unwrap_or(1);
    let width = columns * TILE_WIDTH;
    let mut rgba = vec![0u8; width * rows * cell_height * 4];
    for px in rgba.chunks_exact_mut(4) {
        px.copy_from_slice(&[0, 0, 0, 255]);
    }
    for (i, tile) in tiles.iter().enumerate() {
        let (ox, oy) = ((i % columns) * TILE_WIDTH, (i / columns) * cell_height);
        for y in 0..tile.height {
            let dst = ((oy + y) * width + ox) * 4;
            rgba[dst..dst + tile.width * 4]
                .copy_from_slice(&tile.rgba[y * tile.width * 4..(y + 1) * tile.width * 4]);
        }
    }
    Tile::from_rgba(width, rows * cell_height, rgba).write(path)
}

pub fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let (out, only) = match args.as_slice() {
        [out] => (out, None),
        [out, ids] => (out, Some(ids.clone())),
        _ => return Err("--cockpit-sheet OUTPUT_DIRECTORY [ID,ID,...]".into()),
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    let theater = std::env::var("TORE_PREVIEW_THEATER").unwrap_or_else(|_| "UKR".into());
    let overrides = Overrides {
        time: Some([13, 0]),
        wind: None,
        cloud_altitude: Some(0),
        redrawn_airports: crate::scenery::redrawn_airports()?,
        city_fill: crate::scenery::city_fill()?,
        city_fill_cap: crate::scenery::city_fill_cap()?,
    };
    let world = Terrain::for_mission(resources, &theater, None, &overrides)?;
    let mut scenery = Scenery::build(resources, &world)?;
    let mut gpu = pollster::block_on(Gpu::new(&scenery))?;
    let art = CombatArt::load(resources)?;
    let wanted: Option<Vec<AircraftId>> = match &only {
        Some(list) => Some(
            list.split(',')
                .map(|s| AircraftId::parse(s).map_err(|e| format!("{s}: {e}")))
                .collect::<Result<_, _>>()?,
        ),
        None => None,
    };
    let mut frames = Vec::new();
    let mut art_tiles = Vec::new();
    let mut summary = String::new();
    for id in AircraftId::ALL {
        if wanted.as_ref().is_some_and(|w| !w.contains(&id)) {
            continue;
        }
        let airframe = crate::aircraft::Airframe::load(resources, id)?;
        let name = format!("{id:?}").to_ascii_lowercase();
        let source = &airframe.sprites[id.cockpit()];
        let masks = crate::mirrors::masks(source, id);
        let label = format!(
            "{id:?} {} ART {} {}",
            id.cockpit_stem(),
            id.cockpit()
                .trim_start_matches('~')
                .trim_end_matches(".PIC"),
            describe(masks.as_ref())
        );
        summary.push_str(&format!(
            "{name}: stem {} cockpit {} art {}x{} {}\n",
            id.stem(),
            id.cockpit(),
            source.width,
            source.height,
            describe(masks.as_ref())
        ));
        if let Some(m) = &masks {
            for (rect, tag) in m.rects.iter().zip(["centre", "left", "right"]) {
                if rect[2] > 0. {
                    summary.push_str(&format!(
                        "    {tag}: x {} y {} w {} h {}\n",
                        rect[0], rect[1], rect[2], rect[3]
                    ));
                }
            }
        }
        // Level flight 3,000 ft over the theater start.
        let mut state = airframe.start(&world);
        let ground = f64::from(world.height(state.position[0] as f32, state.position[2] as f32));
        state.position[1] = ground.max(0.) + 3_000.;
        state.gear = 0.;
        state.flaps = 0.;
        state.yaw = 0.;
        state.pitch = 0.;
        state.bank = 0.;
        let camera: Camera = airframe.camera(&state, 0, Default::default());
        scenery.resolve_palette(&world, camera.position[1]);
        scenery.set_origin(camera.position);
        let device = &gpu.device;
        let queue = &gpu.queue;
        gpu.cockpit
            .prepare(device, queue, source, &airframe.cockpit_pic, id);
        let palette = airframe.cockpit_palette(&world, &scenery, camera.position[1], 0);
        gpu.cockpit.weather(queue, &palette);
        gpu.cockpit
            .update(queue, [WIDTH, HEIGHT], &state, &camera, true, false, &[]);
        let devices = tore_sim::combat::countermeasures::Devices::default();
        let empty = tore_sim::combat::smoke::Smoke::default();
        gpu.sim.vapor(device, queue, &[]);
        gpu.sim
            .smoke(device, queue, &art.smoke, [&empty, &empty], &devices);
        gpu.sim.effects(device, queue, &art.effects, &[], &[]);
        gpu.sim
            .emitters(queue, &devices, &[], &crate::gun_flash::Drawn::default());
        gpu.sim.combat(
            device,
            queue,
            &crate::render_snapshot::combat_geometry(
                &RenderSnapshot::default(),
                &art,
                &airframe,
                &state,
                &camera,
                &world,
                &scenery,
            ),
        );
        if gpu.cockpit.mirrors_visible {
            let mirror = crate::mirrors::camera(&state);
            gpu.sim.aircraft(
                device,
                queue,
                &airframe,
                &airframe.vertices(&state, &mirror, &world, &scenery),
            );
            gpu.mirror(&mirror, &world, &scenery);
        }
        gpu.sim.aircraft(
            &gpu.device,
            &gpu.queue,
            &airframe,
            &airframe.vertices(&state, &camera, &world, &scenery),
        );
        gpu.sim.hide_aircraft();
        let pixels = gpu.pixels(&camera, &world, &scenery)?;
        let frame = Tile::from_rgba(WIDTH as usize, HEIGHT as usize, pixels).labelled(
            &airframe.font,
            &label,
            2,
        );
        frame.write(&out.join(format!("{name}-cockpit.png")))?;
        let tile = mask_tile(source, masks.as_ref()).labelled(&airframe.font, &label, 1);
        tile.write(&out.join(format!("{name}-art-masks.png")))?;
        frames.push(frame.scaled(TILE_WIDTH));
        art_tiles.push(tile.scaled(TILE_WIDTH));
        println!("Cockpit sheet: {name} {}", describe(masks.as_ref()));
        let _ = Basis::new(0., 0., 0.);
    }
    sheet(&frames, &out.join("sheet-cockpits.png"))?;
    sheet(&art_tiles, &out.join("sheet-art-masks.png"))?;
    std::fs::write(out.join("summary.txt"), summary)?;
    Ok(())
}
/// `--dome-sheet OUT_DIR`: the radar dome aircraft (E-3, IL-76, E-2) from
/// above and behind at several ticks, so the dome's turn shows. One frame per
/// tick and a labelled row per aircraft in `sheet-domes.png`.
pub fn domes() -> AppResult<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let [out] = args.as_slice() else {
        return Err("--dome-sheet OUTPUT_DIRECTORY".into());
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    let theater = std::env::var("TORE_PREVIEW_THEATER").unwrap_or_else(|_| "UKR".into());
    let overrides = Overrides {
        time: Some([13, 0]),
        wind: None,
        cloud_altitude: Some(0),
        redrawn_airports: crate::scenery::redrawn_airports()?,
        city_fill: crate::scenery::city_fill()?,
        city_fill_cap: crate::scenery::city_fill_cap()?,
    };
    let world = Terrain::for_mission(resources, &theater, None, &overrides)?;
    let mut scenery = Scenery::build(resources, &world)?;
    let mut gpu = pollster::block_on(Gpu::new(&scenery))?;
    let art = CombatArt::load(resources)?;
    // Eight ticks, 100 apart: 50 degrees of dome turn between frames.
    let ticks = [0u64, 100, 200, 300, 400, 500];
    let mut rows = Vec::new();
    for (id, pivot) in [
        (AircraftId::E3, crate::variety_rotors::E3_DOME.pivot),
        (AircraftId::Il76, crate::variety_rotors::IL76_DOME.pivot),
        (AircraftId::E2, crate::variety_rotors::E2_DOME.pivot),
    ] {
        let airframe = crate::aircraft::Airframe::load(resources, id)?;
        let name = id.stem().to_ascii_lowercase();
        let mut row = Vec::new();
        for tick in ticks {
            let mut state = airframe.start(&world);
            let ground =
                f64::from(world.height(state.position[0] as f32, state.position[2] as f32));
            state.position[1] = ground.max(0.) + 3_000.;
            state.gear = 0.;
            state.flaps = 0.;
            state.yaw = 0.;
            state.pitch = 0.;
            state.bank = 0.;
            state.engine = true;
            state.ticks = tick;
            let basis = Basis::new(state.yaw, state.pitch, state.bank);
            let scale = f64::from(airframe.animation_scale());
            // The dome in the world: source X right, Y forward, Z up.
            let dome: [f64; 3] = std::array::from_fn(|i| {
                state.position[i]
                    + (basis.right[i] * f64::from(pivot[0])
                        + basis.forward[i] * f64::from(pivot[1])
                        + basis.up[i] * f64::from(pivot[2]))
                        * scale
            });
            let mut camera = Camera::new();
            // 110 ft above and 90 ft behind-left of the dome.
            camera.position = std::array::from_fn(|i| {
                dome[i] + basis.up[i] * 75. - basis.forward[i] * 70. - basis.right[i] * 45.
            });
            let d: [f64; 3] = std::array::from_fn(|i| dome[i] - camera.position[i]);
            camera.yaw = d[0].atan2(d[2]) as f32;
            camera.pitch = d[1].atan2(d[0].hypot(d[2])) as f32;
            camera.zoom = 1.;
            scenery.resolve_palette(&world, camera.position[1]);
            scenery.set_origin(camera.position);
            let device = &gpu.device;
            let queue = &gpu.queue;
            let devices = tore_sim::combat::countermeasures::Devices::default();
            let empty = tore_sim::combat::smoke::Smoke::default();
            gpu.sim.vapor(device, queue, &[]);
            gpu.sim
                .smoke(device, queue, &art.smoke, [&empty, &empty], &devices);
            gpu.sim.effects(device, queue, &art.effects, &[], &[]);
            gpu.sim
                .emitters(queue, &devices, &[], &crate::gun_flash::Drawn::default());
            gpu.sim.combat(
                device,
                queue,
                &crate::render_snapshot::combat_geometry(
                    &RenderSnapshot::default(),
                    &art,
                    &airframe,
                    &state,
                    &camera,
                    &world,
                    &scenery,
                ),
            );
            gpu.sim.aircraft(
                device,
                queue,
                &airframe,
                &airframe.vertices(&state, &camera, &world, &scenery),
            );
            let pixels = gpu.pixels(&camera, &world, &scenery)?;
            let label = format!(
                "{} TICK {tick} ({} DEG)",
                id.stem(),
                tick * 6 * 360 / (60 * 120)
            );
            let frame = Tile::from_rgba(WIDTH as usize, HEIGHT as usize, pixels).labelled(
                &airframe.font,
                &label,
                2,
            );
            frame.write(&out.join(format!("{name}-dome-t{tick:03}.png")))?;
            row.push(frame.scaled(TILE_WIDTH));
            println!("Dome sheet: {name} tick {tick}");
        }
        rows.push(row);
    }
    let flat: Vec<Tile> = rows.into_iter().flatten().collect();
    sheet_columns(&flat, ticks.len(), &out.join("sheet-domes.png"))
}
