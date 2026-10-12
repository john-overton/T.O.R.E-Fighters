//! `--flight-map-preview OUT_DIR`: offscreen renders of the in-flight map
//! with the mission's markup, for docs/spec/flight-map.md. A development aid:
//! no window and no GPU. It builds one Quick Mission with a ground target and
//! an enemy wing from the imported data, reads the mission's markup from the
//! world as built (the same function the creator's map and a networked game
//! use), then draws the map the way a flight does at several views: following
//! the player, zoomed out to the whole theater, zoomed onto the objective
//! area, scrolled by hand, with the mission layer switched off, as the Redfor
//! pilot sees it, and with the player beyond the theater's edge.
//!
//! ```text
//! tore-app --flight-map-preview OUT_DIR
//! ```
use crate::{AppResult, aircraft::Airframe, flight_map::Map, menu::Sprite, scenery::Scenery};
use std::{collections::BTreeMap, path::Path};
use tore_formats::aircraft::AircraftId;
use tore_sim::combat::live::{DEFAULT_OWNSHIP_SIDE, Side};
use tore_sim::sensors::FEET_PER_NAUTICAL_MILE as NMI;
use tore_world::{
    mission::{Defense, MissionSpec},
    world::{Seating, World},
};

const PLAYER: u32 = 0;

/// The sprites the in-flight map draws with: every imported picture in the
/// menus' palette, and the theaters' map pictures, as the game keeps them.
fn sprites(assets: &crate::assets::Assets) -> AppResult<BTreeMap<String, Sprite>> {
    let palette: [[u8; 3]; 256] = assets
        .pics
        .get("QUIKMIS3.PIC")
        .ok_or("the import has no QUIKMIS3.PIC")?
        .palette
        .clone()
        .try_into()
        .map_err(|_| "quick mission palette missing")?;
    let mut sprites = BTreeMap::new();
    for (name, p) in &assets.pics {
        sprites.insert(
            name.clone(),
            Sprite {
                width: p.width,
                height: p.height,
                rgba: p.rgba(&palette),
                glyphs: p.glyphs.clone(),
            },
        );
    }
    for (name, bytes) in &assets.theater_resources {
        if name.ends_with(".T2") {
            let theater = tore_formats::theater::Theater::parse(bytes)?;
            if let Some(bytes) = assets.theater_resources.get(&theater.map) {
                let p = tore_formats::Pic::parse(bytes)?;
                sprites.insert(
                    theater.map,
                    Sprite {
                        width: p.width,
                        height: p.height,
                        rgba: p.rgba(&palette),
                        glyphs: p.glyphs,
                    },
                );
            }
        }
    }
    Ok(sprites)
}

/// The mission the previews fly: Blue's F/A-18D (airborne) against a pair of
/// enemy aircraft, with the Ukraine city as the ground target.
fn spec() -> AppResult<MissionSpec> {
    let mut spec = MissionSpec::new("UKR", AircraftId::F18);
    spec.wings[3].count = 2;
    spec.wings[3].aircraft = AircraftId::F18;
    spec.separation_nm = 100;
    spec.ground_target = Some("QUCITY".into());
    spec.aaa = Defense::from_level(1).ok_or("defense level")?;
    spec.sam = Defense::from_level(1).ok_or("defense level")?;
    spec.surface_seed = 7;
    spec.validate()?;
    Ok(spec)
}

pub fn run() -> AppResult<()> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let Some(out) = args.first() else {
        return Err("--flight-map-preview OUTPUT_DIRECTORY".into());
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let assets = crate::reel::load_assets()?;
    let resources = assets.theater_resources.clone();
    let sprites = sprites(&assets)?;
    let spec = spec()?;
    let mut world = World::new(&spec, &resources, Seating::SinglePlayer)?;
    let markup = crate::quick_mission::map::mission_plan(&world, false, None);
    let scenery = Scenery::build(&resources, &world.terrain)?;
    let font = Airframe::load(&resources, AircraftId::F18)?.font;
    let size = [
        world.terrain.theater.cols as f64 * f64::from(tore_formats::theater::CELL_FEET),
        world.terrain.theater.rows as f64 * f64::from(tore_formats::theater::CELL_FEET),
    ];
    println!(
        "Flight map preview: theater {:.0} by {:.0} nm, {} markers, objective {:?}",
        size[0] / NMI,
        size[1] / NMI,
        markup.markers.len(),
        markup
            .objective
            .map(|(c, r)| (c.map(|v| (v / NMI).round()), (r / NMI).round())),
    );
    for marker in &markup.markers {
        println!(
            "  {} at {:.0}, {:.0} nm",
            marker.label,
            marker.position[0] / NMI,
            marker.position[2] / NMI
        );
    }
    let start = world.cockpits[0].flight.clone();
    let objective = markup
        .objective
        .ok_or("the mission has no objective area")?;

    // Each preview: a name, where the player is, the viewer's side, the keys
    // and clicks the pilot made, and whether the markup is drawn.
    struct Shot {
        name: &'static str,
        player: [f64; 3],
        own: Side,
        keys: &'static [&'static str],
        /// Clicks (x, y) on the map's reference canvas.
        clicks: &'static [(f64, f64)],
        markup: bool,
    }
    let near_objective = [
        objective.0[0] - 7. * NMI,
        start.position[1],
        objective.0[1] - 5. * NMI,
    ];
    let beyond = [size[0] + 25. * NMI, start.position[1], size[1] * 0.4];
    let shots = [
        Shot {
            name: "follow",
            player: start.position,
            own: DEFAULT_OWNSHIP_SIDE,
            keys: &[],
            clicks: &[],
            markup: true,
        },
        Shot {
            name: "wide",
            player: start.position,
            own: DEFAULT_OWNSHIP_SIDE,
            keys: &["-", "-", "-", "-", "-"],
            clicks: &[],
            markup: true,
        },
        Shot {
            name: "objective",
            player: near_objective,
            own: DEFAULT_OWNSHIP_SIDE,
            keys: &["+"],
            clicks: &[],
            markup: true,
        },
        Shot {
            name: "close",
            player: near_objective,
            own: DEFAULT_OWNSHIP_SIDE,
            keys: &["+", "+", "+"],
            clicks: &[],
            markup: true,
        },
        Shot {
            name: "scrolled",
            player: start.position,
            own: DEFAULT_OWNSHIP_SIDE,
            keys: &["ArrowRight", "ArrowRight", "ArrowUp"],
            clicks: &[],
            markup: true,
        },
        Shot {
            name: "red",
            player: start.position,
            own: Side(2),
            keys: &["-", "-"],
            clicks: &[],
            markup: true,
        },
        Shot {
            name: "layer-off",
            player: start.position,
            own: DEFAULT_OWNSHIP_SIDE,
            keys: &["-", "-"],
            // The MISSION button, pressed and released.
            clicks: &[(510., 300.)],
            markup: true,
        },
        Shot {
            name: "off-map",
            player: beyond,
            own: DEFAULT_OWNSHIP_SIDE,
            keys: &["-", "-", "-", "-"],
            clicks: &[],
            markup: true,
        },
    ];
    for shot in shots {
        let mut state = start.clone();
        state.position = shot.player;
        world.cockpits[0].flight = state.clone();
        let readout = world
            .combat
            .cockpit_readout(
                PLAYER,
                tore_world::combat::launcher(&state),
                world.ai_wings.as_ref(),
                world.cockpits.first(),
            )
            .ok_or("no readout")?;
        let mut map = Map::default();
        let mut pixels = vec![0_u8; 640 * 480 * 4];
        let draw = |map: &mut Map, pixels: &mut Vec<u8>| {
            map.draw(
                pixels,
                &world.terrain,
                &scenery,
                &state,
                &readout,
                shot.own,
                &font,
                &sprites,
                shot.markup.then_some(&markup),
            );
        };
        // The first draw makes the view; the keys and clicks come after it.
        draw(&mut map, &mut pixels);
        for key in shot.keys {
            map.key(key);
        }
        for &click in shot.clicks {
            map.pointer(Some(click), true);
            map.pointer(Some(click), false);
        }
        draw(&mut map, &mut pixels);
        // The plot (x 12 to 470, y 30 to 420) must never show the void the
        // map draws beyond the theater: no black borders at any zoom.
        let void = (30..420)
            .flat_map(|y| (12..470).map(move |x| (y * 640 + x) * 4))
            .filter(|&i| pixels[i..i + 3] == [18, 18, 18])
            .count();
        let path = out.join(format!("{}.png", shot.name));
        std::fs::write(&path, crate::replay::png::encode_rgba(640, 480, &pixels)?)?;
        println!(
            "Flight map preview: {} (void pixels {void})",
            path.display()
        );
        if void > 50 {
            return Err(format!("{}: {void} void pixels show in the map's plot", shot.name).into());
        }
    }
    Ok(())
}
