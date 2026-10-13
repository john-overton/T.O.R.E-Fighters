//! `--hud-a2g-preview OUT`: review sheets of the HUD's bomb and rocket sights
//! (docs/spec/air-to-ground.md, "The HUD in air-to-ground modes"), drawn as
//! the cockpit draws the HUD layer, without a window.
//!
//! Each frame builds the Quick Mission of a ground target, places the
//! aircraft (invulnerable) where the frame wants it, steps the whole world
//! so combat works out the sight, and draws the flight HUD and the weapon
//! HUD over a plain sky and ground with the target marked. Frames: CCIP
//! beyond, near and inside the bomb's range, off the HUD, LOW and FRAG, on an
//! F/A-18D (dotted fall line) and an F-16C (dot trail); the level "T" far
//! out, 10 s, 2 s and at the release, off track and OVERFLY on an F-4B; the
//! rocket pipper on an A-4E. Each is written twice its size as `NAME.png`,
//! and the lot as contact sheets `sheet-N.png`.
use super::{Options, Scene};
use crate::{AppResult, hud, surface_dump, weapon_hud};
use tore_formats::{aircraft::AircraftId, font::Font};
use tore_sim::{
    attitude::{Basis, Vector},
    combat::{ballistics::ReleaseState, delivery::DeliveryMode, live::Command},
};
use tore_world::seats::SeatCommand;

const W: usize = 640;
const H: usize = 480;
const HUD_GREEN: [u8; 3] = [72, 255, 96];
const SKY: [u8; 3] = [28, 44, 72];
const GROUND: [u8; 3] = [46, 52, 34];
const KNOT: f64 = 1.687_8;

/// Where a frame puts the aircraft, relative to the target.
#[derive(Clone, Copy)]
enum Place {
    /// On the dive line through the release point that puts the round's mean
    /// path on the target from `altitude`, `back` feet before that point.
    Dive { dive: f64, altitude: f64, back: f64 },
    /// Level at `altitude`, `seconds` of flying before the release point,
    /// `right` feet off the line to the target.
    Level {
        altitude: f64,
        seconds: f64,
        right: f64,
    },
}

struct Shot {
    name: &'static str,
    caption: &'static str,
    aircraft: AircraftId,
    load: &'static str,
    place: Place,
    knots: f64,
    level: bool,
    /// Press the trigger at the pose and fly on this many seconds first.
    after: f64,
}

fn shots() -> Vec<Shot> {
    let ccip = |name, caption, aircraft, dive, altitude, back| Shot {
        name,
        caption,
        aircraft,
        load: "MK82",
        place: Place::Dive {
            dive,
            altitude,
            back,
        },
        knots: 450.,
        level: false,
        after: 0.,
    };
    let lvl = |name, caption, seconds, right| Shot {
        name,
        caption,
        aircraft: AircraftId::F4B,
        load: "MK82",
        place: Place::Level {
            altitude: 5_000.,
            seconds,
            right,
        },
        knots: 300.,
        level: true,
        after: 0.,
    };
    let (f18, f16) = (AircraftId::F18, AircraftId::F16C);
    let fab = |shot: Shot| Shot {
        load: "FAB250",
        ..shot
    };
    let after = |shot: Shot, after| Shot { after, ..shot };
    let rocket = |name, caption, place| Shot {
        name,
        caption,
        aircraft: AircraftId::A4E,
        load: "LAU61",
        place,
        knots: 400.,
        level: false,
        after: 0.,
    };
    vec![
        fab(ccip(
            "ccip-f18-beyond",
            "FAB-250 beyond R (10,000 ft): bare ring",
            f18,
            45.,
            4_000.,
            8_000.,
        )),
        ccip(
            "ccip-f18-near-max",
            "CCIP near R: short arc",
            f18,
            30.,
            3_000.,
            13_000.,
        ),
        ccip("ccip-f18-mid", "CCIP mid range", f18, 30., 3_000., 7_000.),
        ccip(
            "ccip-f18-in-range",
            "CCIP inside 0.4 R: full arc",
            f18,
            30.,
            2_000.,
            500.,
        ),
        ccip(
            "ccip-f18-off-hud",
            "CCIP level pass: caret",
            f18,
            0.,
            5_000.,
            3_000.,
        ),
        ccip(
            "ccip-f18-low",
            "LOW and FRAG: 300 ft dive",
            f18,
            30.,
            300.,
            0.,
        ),
        ccip("ccip-f18-frag", "FRAG: 400 ft level", f18, 0., 400., 300.),
        after(
            ccip(
                "ccip-f18-imp",
                "CCIP 3 s after a release: IMP",
                f18,
                30.,
                3_000.,
                3_000.,
            ),
            3.,
        ),
        fab(ccip(
            "ccip-f16-beyond",
            "F-16C dot trail, FAB-250 beyond R",
            f16,
            45.,
            4_000.,
            8_000.,
        )),
        ccip(
            "ccip-f16-mid",
            "F-16C dot trail mid range",
            f16,
            30.,
            3_000.,
            7_000.,
        ),
        ccip(
            "ccip-f16-in-range",
            "F-16C dot trail inside 0.4 R",
            f16,
            30.,
            2_000.,
            500.,
        ),
        lvl("lvl-far", "LVL far out: dashed stem", 75., 0.),
        lvl("lvl-10s", "LVL 10 s to release", 10., 0.),
        lvl("lvl-2s", "LVL 2 s to release", 2., 0.),
        lvl("lvl-release", "LVL at the release", -0.15, 0.),
        lvl("lvl-off-track", "LVL target 600 ft right", 10., 600.),
        lvl("lvl-overfly", "LVL OVERFLY", -3., 0.),
        Shot {
            name: "rocket-a4e",
            caption: "Rocket pipper (ARM)",
            aircraft: AircraftId::A4E,
            load: "LAU61",
            place: Place::Dive {
                dive: 10.,
                altitude: 1_500.,
                back: 1_000.,
            },
            knots: 400.,
            level: false,
            after: 0.,
        },
        // The rocket pipper is always shown (John, 2026-10-12): level at
        // 3,000 ft it stands past the rocket's 20 s life, in a steep dive it
        // is a close-in impact on the bore line, climbing it marks where the
        // rocket's life ends.
        rocket(
            "rocket-a4e-level-3000",
            "Rocket pipper, level at 3,000 ft (was missing)",
            Place::Level {
                altitude: 3_000.,
                seconds: 0.,
                right: 0.,
            },
        ),
        rocket(
            "rocket-a4e-shallow-2",
            "Rocket pipper, 2 degrees down from 3,000 ft (was missing)",
            Place::Dive {
                dive: 2.,
                altitude: 3_000.,
                back: 1_000.,
            },
        ),
        rocket(
            "rocket-a4e-steep-65",
            "Rocket pipper, 65 degree dive (shown before too)",
            Place::Dive {
                dive: 65.,
                altitude: 3_000.,
                back: 1_000.,
            },
        ),
        rocket(
            "rocket-a4e-climb",
            "Rocket pipper, climbing 10 degrees: where the life ends",
            Place::Dive {
                dive: -10.,
                altitude: 3_000.,
                back: 1_000.,
            },
        ),
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
        altitude: 5_000.,
        speed_kt: shot.knots,
        dive_deg: 0.,
        heading_deg: 0.,
        seconds: 1.,
        target: 2,
        station: None,
        count: Some(4),
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
        ai: super::ai::Config::new(),
    }
}

pub fn run() -> AppResult<()> {
    let out = std::path::PathBuf::from(
        std::env::args()
            .nth(2)
            .ok_or("--hud-a2g-preview needs an output folder")?,
    );
    std::fs::create_dir_all(&out)?;
    let mut resources = crate::reel::load_assets()?.theater_resources;
    surface_dump::with_retail_surface(&mut resources)?;
    let font = Font::parse(resources.get("HUD11.FNT").ok_or("missing HUD11.FNT")?)?;
    let mut frames = Vec::new();
    for shot in shots() {
        let mut o = options(&shot);
        let scene = match Scene::new(&o, &resources) {
            Ok(scene) => scene,
            Err(_) => {
                // No bomb or pod station to fit it to: station 1.
                o.station = Some(1);
                Scene::new(&o, &resources)?
            }
        };
        let rgba = frame(scene, &shot, &font)?;
        let path = out.join(format!("{}.png", shot.name));
        std::fs::write(
            &path,
            crate::replay::png::encode_rgba(2 * W as u32, 2 * H as u32, &doubled(&rgba))?,
        )?;
        println!("hud-a2g-preview: {}", path.display());
        frames.push(rgba);
    }
    for (index, chunk) in frames.chunks(6).enumerate() {
        let path = out.join(format!("sheet-{}.png", index + 1));
        let (columns, rows) = (3, chunk.len().div_ceil(3));
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
        println!("hud-a2g-preview: {}", path.display());
    }
    Ok(())
}

/// Where the shot puts the aircraft and its pitch.
fn pose(scene: &Scene, shot: &Shot, speed: f64) -> AppResult<(Vector, f64)> {
    let target = scene.target_row().ok_or("no target row")?.position;
    let point = [target[0], scene.ground(target[0], target[2]), target[2]];
    let weapon = &scene.station().weapon;
    let release_speed = tore_sim::combat::launch_speed(&weapon.movement, (speed * 256.) as i32)
        .map_or(0., f64::from);
    Ok(match shot.place {
        Place::Dive {
            dive,
            altitude,
            back,
        } => {
            let pitch = -dive.to_radians();
            let forward = [0., pitch.sin(), pitch.cos()];
            let through = if tore_sim::combat::ballistics::is_bomb(weapon) {
                ReleaseState::onto(point, forward, release_speed, altitude).position
            } else {
                let run = altitude / pitch.abs().tan().max(1e-3);
                [point[0], point[1] + altitude, point[2] - run]
            };
            (
                std::array::from_fn(|i| through[i] - forward[i] * back),
                pitch,
            )
        }
        Place::Level {
            altitude,
            seconds,
            right,
        } => {
            let through = ReleaseState::onto(point, [0., 0., 1.], release_speed, altitude).position;
            (
                [through[0] - right, through[1], through[2] - speed * seconds],
                0.,
            )
        }
    })
}

/// Places the aircraft, lets combat solve the sight and draws the HUD over a
/// backdrop.
fn frame(mut scene: Scene, shot: &Shot, font: &Font) -> AppResult<Vec<u8>> {
    let speed = shot.knots * KNOT;
    if shot.level {
        // Easy targeting's Ctrl+X designates the nearest hostile ground
        // object and keeps it while it stands.
        scene.world.combat.state.cheats.easy_targeting = true;
        let (position, pitch) = pose(&scene, shot, speed)?;
        scene.fly(position, 0., pitch, speed);
        scene.queue(SeatCommand::Combat(Command::NearestGround));
        scene.step(false)?;
        if let Some(id) = scene.world.combat.state.own().designated() {
            scene.target = id;
        }
    }
    let (position, pitch) = pose(&scene, shot, speed)?;
    // Select the store, then hold the pose for the step the sight is solved on.
    for _ in 0..4 {
        scene.fly(position, 0., pitch, speed);
        scene.select();
        scene.step(false)?;
    }
    if shot.level && scene.world.combat.state.own().delivery_mode != DeliveryMode::Level {
        scene.queue(SeatCommand::Combat(Command::ToggleSeekerMode));
    }
    scene.fly(position, 0., pitch, speed);
    scene.step(shot.after > 0.)?;
    let forward = [0., pitch.sin(), pitch.cos()];
    let mut position = position;
    for _ in 0..(shot.after * 120.) as u64 {
        position = std::array::from_fn(|i| position[i] + forward[i] * speed / 120.);
        scene.fly(position, 0., pitch, speed);
        scene.step(false)?;
    }
    scene.fly(position, 0., pitch, speed);
    let target = scene.target_row().ok_or("no target row")?.position;
    let ground = scene.ground(position[0], position[2]);
    let world = &scene.world;
    let picture = world.combat.render_snapshot();
    let frame = world
        .flight_frame(tore_world::seats::SeatId(0), None, picture, &[])
        .ok_or("no flight frame")?;
    let s = frame.presented();
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
    weapon_hud::draw(
        &mut layer, &frame, font, HUD_GREEN, 1., false, true, None, false,
    );
    let sight = frame.readout.bomb_sight.clone();
    println!(
        "hud-a2g-preview: {} {:?} mode {:?} arc {:?} range {:?} level {:?} warnings {:?}",
        shot.name,
        shot.aircraft,
        sight.as_ref().map(|s| s.mode),
        sight.as_ref().and_then(|s| s.ccip).map(|c| c.arc),
        sight
            .as_ref()
            .and_then(|s| s.ccip)
            .map(|c| c.range_ft.round()),
        sight.as_ref().and_then(|s| s.level).map(|l| (
            l.along_ft.round(),
            l.cross_ft.round(),
            (l.release_seconds * 10.).round() / 10.
        )),
        sight.as_ref().map(|s| s.warnings.clone()),
    );
    let mut rgba = backdrop(s, target);
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
    caption.text(
        font,
        &format!("{}: {}", aircraft_name(shot.aircraft), shot.caption),
        8,
        8,
    );
    caption.text(font, "target marked tan", 8, 20);
    Ok(rgba)
}

/// Sky over ground split at the horizon, and the target as a tan mark.
pub(super) fn backdrop(s: &crate::flight::State, target: Vector) -> Vec<u8> {
    let mut rgba = vec![0u8; W * H * 4];
    let focal = 240. * 3f64.sqrt();
    let horizon = |x: f64| {
        let bearing = ((x - 320.) / focal).atan();
        hud::project(s.pitch, s.bank, bearing, 0., 1.).map_or(-1e9, |p| p.1)
    };
    for x in 0..W {
        let edge = horizon(x as f64);
        for y in 0..H {
            let color = if (y as f64) < edge { SKY } else { GROUND };
            let at = (y * W + x) * 4;
            rgba[at..at + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
        }
    }
    let basis = Basis::new(s.yaw, s.pitch, s.bank);
    let rel: Vector = std::array::from_fn(|i| target[i] - s.position[i]);
    let along = |axis: Vector| rel.iter().zip(axis).map(|(a, b)| a * b).sum::<f64>();
    let z = along(basis.forward);
    if z > 1. {
        let x = 320. + focal * along(basis.right) / z;
        let y = 240. - focal * along(basis.up) / z;
        for dy in -1..=1 {
            for dx in -2..=2 {
                let (px, py) = (x.round() as i64 + dx, y.round() as i64 + dy);
                if (0..W as i64).contains(&px) && (0..H as i64).contains(&py) {
                    let at = (py as usize * W + px as usize) * 4;
                    rgba[at..at + 4].copy_from_slice(&[200, 170, 120, 255]);
                }
            }
        }
    }
    rgba
}

fn aircraft_name(id: AircraftId) -> &'static str {
    match id {
        AircraftId::F18 => "F/A-18D",
        AircraftId::F16C => "F-16C",
        AircraftId::F4B => "F-4B",
        AircraftId::A4E => "A-4E",
        _ => "aircraft",
    }
}

pub(super) fn doubled(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len() * 4);
    for y in 0..2 * H {
        for x in 0..2 * W {
            let at = ((y / 2) * W + x / 2) * 4;
            out.extend_from_slice(&rgba[at..at + 4]);
        }
    }
    out
}
