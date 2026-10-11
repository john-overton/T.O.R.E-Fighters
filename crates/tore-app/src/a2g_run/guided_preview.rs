//! `--hud-guided-preview OUT`: review frames of the guided weapons' release
//! range scale (docs/spec/air-to-ground.md, "Guided bomb release range
//! indicator"; slice W1), drawn as the cockpit draws the HUD layer, without a
//! window.
//!
//! Each frame builds the Quick Mission of a ground target, designates the
//! nearest object (Easy targeting's Ctrl+X), places the aircraft level
//! `range` feet short of it, steps the whole world so combat works out the
//! guided sight, and draws the flight HUD and the weapon HUD over a plain sky
//! and ground with the target marked. Frames: an F-16C's GBU-30 (JDAM) far
//! beyond its reach, at the far edge, in the middle, inside its near edge and
//! with nothing designated; its GBU-10 lasing in reach and with the laser
//! off (NO LASER); an Su-25's AS-14 on its zone. Each is written twice its
//! size as `NAME.png`, and the lot as contact sheets `guided-sheet-N.png`.
use super::{Options, Scene, guided, preview};
use crate::{AppResult, hud, surface_dump, weapon_hud};
use tore_formats::{aircraft::AircraftId, font::Font};
use tore_sim::combat::live::Command;
use tore_world::seats::SeatCommand;

const W: usize = 640;
const H: usize = 480;
const HUD_GREEN: [u8; 3] = [72, 255, 96];
const KNOT: f64 = 1.687_8;

struct Shot {
    name: &'static str,
    caption: &'static str,
    aircraft: AircraftId,
    load: &'static str,
    altitude: f64,
    knots: f64,
    /// Ground range to the target, feet.
    range: f64,
    designate: bool,
    laser_off: bool,
    /// Release one (a plain press past the bar) and draw the HUD 3 s on.
    release: bool,
}

fn shots() -> Vec<Shot> {
    let jdam = |name, caption, range, designate| Shot {
        name,
        caption,
        aircraft: AircraftId::F16C,
        load: "GBU30",
        altitude: 12_000.,
        knots: 450.,
        range,
        designate,
        laser_off: false,
        release: false,
    };
    vec![
        jdam(
            "gbu30-far",
            "GBU-30 at 12,000 ft, target 5 nm: OUT OF REACH",
            30_000.,
            true,
        ),
        jdam(
            "gbu30-edge",
            "GBU-30, target at the far edge: IN RNG",
            18_300.,
            true,
        ),
        jdam("gbu30-mid", "GBU-30, target mid reach", 15_000., true),
        jdam(
            "gbu30-close",
            "GBU-30, target inside the near edge",
            9_000.,
            true,
        ),
        jdam(
            "gbu30-no-target",
            "GBU-30 with nothing designated",
            15_000.,
            false,
        ),
        Shot {
            name: "gbu10-lasing",
            caption: "GBU-10 on its own spot, mid reach",
            load: "GBU10",
            ..jdam("", "", 15_000., true)
        },
        Shot {
            name: "gbu10-no-laser",
            caption: "GBU-10, laser off by Ctrl+L: NO LASER",
            load: "GBU10",
            laser_off: true,
            ..jdam("", "", 15_000., true)
        },
        Shot {
            name: "as14-zone",
            caption: "Su-25 AS-14 at 3,000 ft: its zone",
            aircraft: AircraftId::Su25,
            load: "AS14",
            altitude: 3_000.,
            knots: 400.,
            range: 20_000.,
            designate: true,
            laser_off: false,
            release: false,
        },
        Shot {
            name: "gbu30-in-flight",
            caption: "GBU-30 released, 3 s on: TTI row",
            release: true,
            ..jdam("", "", 14_000., true)
        },
        Shot {
            name: "gbu10-in-flight",
            caption: "GBU-10 released on its spot, 3 s on: LSR and TTI rows",
            load: "GBU10",
            release: true,
            ..jdam("", "", 14_000., true)
        },
    ]
}

fn options(shot: &Shot) -> Options {
    Options {
        theater: "TVIET".into(),
        stem: "QTTRUCK".into(),
        aircraft: shot.aircraft,
        load: Some(super::record_name(shot.load)),
        attack: "preview".into(),
        from_nm: 1.,
        altitude: shot.altitude,
        speed_kt: shot.knots,
        dive_deg: 0.,
        heading_deg: 0.,
        seconds: 1.,
        target: 0,
        station: Some(2),
        count: Some(2),
        hold: 0.,
        repeat: 1,
        gap: 0.5,
        defenses: (0, 0),
        seed: 1,
        unit: None,
        designate: None,
        break_at: None,
        skill: None,
        render: None,
        record: None,
        verify: false,
    }
}

pub fn run() -> AppResult<()> {
    let out = std::path::PathBuf::from(
        std::env::args()
            .nth(2)
            .ok_or("--hud-guided-preview needs an output folder")?,
    );
    std::fs::create_dir_all(&out)?;
    let mut resources = crate::reel::load_assets()?.theater_resources;
    surface_dump::with_retail_surface(&mut resources)?;
    let font = Font::parse(resources.get("HUD11.FNT").ok_or("missing HUD11.FNT")?)?;
    let mut frames = Vec::new();
    for shot in shots() {
        let (scene, _) = guided::scene(&options(&shot), &guided::Extra::default(), &resources)?;
        let rgba = frame(scene, &shot, &font)?;
        let path = out.join(format!("{}.png", shot.name));
        std::fs::write(
            &path,
            crate::replay::png::encode_rgba(2 * W as u32, 2 * H as u32, &preview::doubled(&rgba))?,
        )?;
        println!("hud-guided-preview: {}", path.display());
        frames.push(rgba);
    }
    for (index, chunk) in frames.chunks(4).enumerate() {
        let path = out.join(format!("guided-sheet-{}.png", index + 1));
        let (columns, rows) = (2, chunk.len().div_ceil(2));
        let mut sheet = vec![0u8; columns * W * rows * H * 4];
        for (n, f) in chunk.iter().enumerate() {
            let (cx, cy) = (n % columns, n / columns);
            for y in 0..H {
                let from = y * W * 4;
                let to = ((cy * H + y) * columns * W + cx * W) * 4;
                sheet[to..to + W * 4].copy_from_slice(&f[from..from + W * 4]);
            }
        }
        std::fs::write(
            &path,
            crate::replay::png::encode_rgba((columns * W) as u32, (rows * H) as u32, &sheet)?,
        )?;
        println!("hud-guided-preview: {}", path.display());
    }
    Ok(())
}

/// Designates, places the aircraft, lets combat solve the sight and draws the
/// HUD over a backdrop.
fn frame(mut scene: Scene, shot: &Shot, font: &Font) -> AppResult<Vec<u8>> {
    let speed = shot.knots * KNOT;
    let owner = scene.owner;
    let start = scene.target_row().ok_or("no target row")?.position;
    let pose =
        |target: [f64; 3], ground: f64| [target[0], ground + shot.altitude, target[2] - shot.range];
    let ground = scene.ground(start[0], start[2]);
    scene.fly(pose(start, ground), 0., 0., speed);
    if shot.designate {
        scene.world.combat.state.cheats.easy_targeting = true;
        scene
            .commands
            .push(SeatCommand::Combat(Command::NearestGround));
        scene.step(false)?;
        if let Some(id) = scene
            .world
            .combat
            .state
            .ownship(owner)
            .and_then(|own| own.designated())
        {
            scene.target = id;
        }
    }
    let target = scene.target_row().ok_or("no target row")?.position;
    let ground = scene.ground(target[0], target[2]);
    let position = pose(target, ground);
    for n in 0..240 {
        scene.fly(position, 0., 0., speed);
        scene.select();
        if shot.laser_off && n == 120 {
            scene
                .commands
                .push(SeatCommand::Combat(Command::ToggleLaser));
        }
        scene.step(false)?;
        // Hold until the sight is solved and the blink is lit.
        if n > 180 && scene.world.combat.state.tick() % 60 < 30 {
            break;
        }
    }
    if shot.release {
        scene.fly(position, 0., 0., speed);
        scene.step(true)?;
        for _ in 0..360 {
            scene.fly(position, 0., 0., speed);
            scene.step(false)?;
        }
    }
    scene.fly(position, 0., 0., speed);
    let world = &scene.world;
    let picture = world.combat.render_snapshot();
    let frame = world
        .flight_frame(tore_world::seats::SeatId(0), None, picture, &[])
        .ok_or("no flight frame")?;
    let s = frame.presented();
    let sight = frame.readout.guided;
    println!(
        "hud-guided-preview: {} range {:?} reach {:?} in-reach {:?} advisory {:?} readiness {:?}",
        shot.name,
        sight.and_then(|g| g.ground_range_ft).map(f64::round),
        sight
            .and_then(|g| g.reach)
            .map(|r| (r.minimum_ft.round(), r.maximum_ft.round())),
        sight.map(|g| g.in_reach),
        sight.and_then(|g| g.advisory),
        frame.readout.estimates.readiness,
    );
    let mut layer = vec![0u8; W * H * 4];
    hud::draw(
        &mut layer,
        s,
        font,
        ground,
        None,
        true,
        weapon_hud::active(&frame),
        HUD_GREEN,
        1.,
        None,
        None,
        (s.bank, 1.),
    );
    weapon_hud::draw(&mut layer, &frame, font, HUD_GREEN, 1., false, true, None);
    let mut rgba = preview::backdrop(s, target);
    for (pixel, hud) in rgba.chunks_exact_mut(4).zip(layer.chunks_exact(4)) {
        if hud[3] != 0 {
            pixel[..3].copy_from_slice(&hud[..3]);
        }
    }
    let mut caption = hud::Paint {
        pixels: &mut rgba,
        clip: (0, 0, W as i32, H as i32),
        color: [255, 255, 255, 255],
    };
    let name = match shot.aircraft {
        AircraftId::F16C => "F-16C",
        AircraftId::Su25 => "Su-25",
        _ => "aircraft",
    };
    caption.text(font, &format!("{name}: {}", shot.caption), 8, 8);
    caption.text(font, "target marked tan", 8, 20);
    Ok(rgba)
}
