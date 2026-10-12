//! `--impact-preview OUT_DIR`: offscreen renders of weapon impacts side by
//! side, so their sizes, light and sparks can be compared (slice BF, bugs B8
//! to B11). A row of ground impacts stands on open ground ahead of the
//! theater's start, 20 mm, 105 mm, then 250 lb, 500 lb and 2,000 lb bombs
//! left to right, seen from low beside the row by day and at night, a few
//! moments into the explosions; then close views of a 20 mm and a 105 mm
//! hit, and a night close view of a 20 mm burst. A development aid: no
//! window opens, nothing is simulated, the effects are placed by hand with
//! each record's own explosion type, and drawn the way a picture of the
//! game draws them (sprites, lights, sparks).
use crate::{
    AppResult,
    camera::Camera,
    reel::{Gpu, HEIGHT, WIDTH},
    render_snapshot::CombatArt,
    scenery::Scenery,
    snapshot::{EffectPose, RenderSnapshot},
    terrain::{Overrides, Terrain},
};
use std::path::Path;
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::combat::{blast, live::EffectKind};

/// One impact of the row: its label, the weapon record whose land explosion
/// it shows and the store weight it is drawn for (pounds), when not the
/// record's own.
struct Impact {
    label: &'static str,
    record: &'static str,
    weight: Option<i32>,
}

const ROW: [Impact; 5] = [
    Impact {
        label: "20mm",
        record: "M61.JT",
        weight: None,
    },
    Impact {
        label: "105mm",
        record: "C_105.JT",
        weight: None,
    },
    Impact {
        label: "250lb",
        record: "MK82.JT",
        weight: Some(250),
    },
    Impact {
        label: "500lb",
        record: "MK82.JT",
        weight: None,
    },
    Impact {
        label: "2000lb",
        record: "MK84.JT",
        weight: None,
    },
];

/// Feet between impacts of the row.
const SPACING: f64 = 900.;

/// The effect an impact of `weapon` on open ground shows `elapsed` ticks in.
fn ground_hit(weapon: &Weapon, position: [f64; 3], elapsed: u16) -> Option<EffectPose> {
    let kind = weapon.effects.land_explosion;
    let row = blast::explosion(kind)?;
    let life = u16::from(row.seconds) * 120;
    Some(EffectPose {
        kind: EffectKind::Ground,
        position,
        ticks: life.saturating_sub(elapsed).max(1),
        blast: Some(kind),
    })
}

/// The weapon record `name` from the import, its store weight replaced when
/// `weight` says so.
fn record(
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
    name: &str,
    weight: Option<i32>,
) -> AppResult<Weapon> {
    let bytes = resources
        .get(name)
        .ok_or_else(|| format!("the import has no {name}"))?;
    let mut weapon = Weapon::parse(name, bytes)?;
    if let Some(weight) = weight {
        weapon.weight = weight;
    }
    Ok(weapon)
}

struct Shot {
    name: String,
    effects: Vec<EffectPose>,
    eye: [f64; 3],
    look: [f64; 3],
    fov: f64,
    tick: u64,
}

pub fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let [out] = args.as_slice() else {
        return Err("--impact-preview OUTPUT_DIRECTORY".into());
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    let theater = std::env::var("TORE_PREVIEW_THEATER").unwrap_or_else(|_| "UKR".into());
    let row: Vec<(&Impact, Weapon)> = ROW
        .iter()
        .map(|impact| Ok((impact, record(resources, impact.record, impact.weight)?)))
        .collect::<AppResult<_>>()?;
    for (light, condition, time) in [("day", None, Some([13, 0])), ("night", Some(5), None)] {
        let overrides = Overrides {
            time,
            wind: None,
            cloud_altitude: Some(0),
            redrawn_airports: crate::scenery::redrawn_airports()?,
            city_fill: crate::scenery::city_fill()?,
            city_fill_cap: crate::scenery::city_fill_cap()?,
        };
        let world = Terrain::for_mission(resources, &theater, condition, &overrides)?;
        let mut scenery = Scenery::build(resources, &world)?;
        let mut gpu = pollster::block_on(Gpu::new(&scenery))?;
        let art = CombatArt::load(resources)?;
        let ownship = crate::aircraft::Airframe::load(resources, AircraftId::F18)?;
        let start = ownship.start(&world);
        let base = [start.position[0] + 20_000., 0., start.position[2] + 20_000.];
        let ground = |x: f64, z: f64| f64::from(world.height(x as f32, z as f32)).max(0.);
        let spot = |n: f64| {
            let x = base[0] + n * SPACING;
            [x, ground(x, base[2]), base[2]]
        };
        let middle = spot(2.);
        let mut shots = Vec::new();
        for seconds in [0.1, 0.3, 0.6, 1.0] {
            let elapsed = (seconds * 120.) as u16;
            let effects = row
                .iter()
                .enumerate()
                .filter_map(|(n, (_, weapon))| ground_hit(weapon, spot(n as f64), elapsed))
                .collect();
            shots.push(Shot {
                name: format!("row-t{seconds:.2}s-{light}"),
                effects,
                eye: [middle[0], middle[1] + 300., middle[2] - 2_500.],
                look: [middle[0], middle[1] + 60., middle[2]],
                fov: 80.,
                tick: 10_000 + u64::from(elapsed),
            });
        }
        // Close views: a 20 mm and a 105 mm hit at the same distance.
        for (n, (impact, weapon)) in row.iter().enumerate().take(2) {
            for seconds in [0.05, 0.15, 0.4] {
                let at = spot(n as f64);
                let elapsed = (seconds * 120.) as u16;
                shots.push(Shot {
                    name: format!("close-{}-t{seconds:.2}s-{light}", impact.label),
                    effects: ground_hit(weapon, at, elapsed).into_iter().collect(),
                    eye: [at[0] - 260., at[1] + 60., at[2] - 420.],
                    look: [at[0], at[1] + 15., at[2]],
                    fov: 30.,
                    tick: 10_000 + u64::from(elapsed),
                });
            }
        }
        // A 20 mm burst: eight hits a few feet apart, a tick or two between.
        let (_, cannon) = &row[0];
        let at = spot(0.);
        for seconds in [0.1, 0.25] {
            let elapsed = (seconds * 120.) as u16;
            let effects = (0..8_u16)
                .filter_map(|k| {
                    let along = f64::from(k) * 9.;
                    let position = [
                        at[0] + along,
                        ground(at[0] + along, at[2] + f64::from(k % 3) * 6.),
                        at[2] + f64::from(k % 3) * 6.,
                    ];
                    ground_hit(cannon, position, elapsed.saturating_sub(k * 2))
                })
                .collect();
            shots.push(Shot {
                name: format!("burst-20mm-t{seconds:.2}s-{light}"),
                effects,
                eye: [at[0] - 300., at[1] + 90., at[2] - 520.],
                look: [at[0] + 30., at[1] + 10., at[2]],
                fov: 30.,
                tick: 10_000 + u64::from(elapsed),
            });
        }
        for shot in &shots {
            let mut camera = Camera::new();
            camera.position = shot.eye;
            let floor = ground(camera.position[0], camera.position[2]);
            camera.position[1] = camera.position[1].max(floor + 30.);
            let d: [f64; 3] = std::array::from_fn(|i| shot.look[i] - camera.position[i]);
            camera.yaw = d[0].atan2(d[2]) as f32;
            camera.pitch = d[1].atan2(d[0].hypot(d[2])) as f32;
            camera.zoom = (30_f64.to_radians().tan() / (shot.fov / 2.).to_radians().tan()) as f32;
            camera.near_clip = 1.;
            scenery.resolve_palette(&world, camera.position[1]);
            scenery.set_origin(camera.position);
            let picture = RenderSnapshot {
                tick: shot.tick,
                effects: shot.effects.clone(),
                ..RenderSnapshot::default()
            };
            // The lights the picture's effects give, as a flight draws them.
            let mut tracker = crate::gun_flash::Tracker::default();
            let now = picture.tick as f64;
            tracker.observe(&picture, now, |_| None);
            let drawn = tracker.draw(now, |_| None);
            let device = &gpu.device;
            let queue = &gpu.queue;
            let devices = tore_sim::combat::countermeasures::Devices::default();
            let empty = tore_sim::combat::smoke::Smoke::default();
            gpu.sim.vapor(device, queue, &[]);
            gpu.sim
                .smoke(device, queue, &art.smoke, [&empty, &empty], &devices);
            gpu.sim
                .effects(device, queue, &art.effects, &picture.effects, &[]);
            gpu.sim.emitters(queue, &devices, &[], &drawn);
            gpu.sim.combat(
                device,
                queue,
                &crate::render_snapshot::combat_geometry(
                    &picture, &art, &ownship, &start, &camera, &world, &scenery,
                ),
            );
            gpu.sim.aircraft(device, queue, &ownship, &[]);
            let pixels = gpu.pixels(&camera, &world, &scenery)?;
            let path = out.join(format!("{}.png", shot.name));
            std::fs::write(
                &path,
                crate::replay::png::encode_rgba(WIDTH, HEIGHT, &pixels)?,
            )?;
            println!(
                "Impact preview: {} ({} effects, {} lights)",
                path.display(),
                shot.effects.len(),
                drawn.lights.len()
            );
        }
    }
    Ok(())
}
