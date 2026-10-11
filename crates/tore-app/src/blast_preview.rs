//! `--blast-preview OUT_DIR`: offscreen renders of a large ground
//! explosion and its shockwave ring through the explosion's two seconds,
//! from a strike aircraft's height and from low beside it, by day and at
//! dusk; and the white spray ring of a large water explosion. A development
//! aid for docs/spec/explosions.md#shockwave: no window opens, nothing is
//! simulated, the effects are placed by hand.
use crate::{
    AppResult,
    camera::Camera,
    reel::{Gpu, HEIGHT, WIDTH},
    render_snapshot::CombatArt,
    scenery::Scenery,
    snapshot::{EffectPose, MarkPose, RenderSnapshot},
    terrain::{Overrides, Terrain},
};
use std::path::Path;
use tore_formats::aircraft::AircraftId;
use tore_sim::combat::{blast::MarkKind, live::EffectKind};

/// The explosions' life: types 21 to 23 and 34 to 37 last two seconds.
const LIFE: u16 = 240;

/// One picture: the explosion type, seconds after the blast, and the
/// camera's place relative to the blast (east, up, north, in feet).
struct Shot {
    name: &'static str,
    kind: u8,
    seconds: f64,
    eye: [f64; 3],
    fov: f64,
    crater: bool,
}

pub fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let [out] = args.as_slice() else {
        return Err("--blast-preview OUTPUT_DIRECTORY".into());
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    let theater = std::env::var("TORE_PREVIEW_THEATER").unwrap_or_else(|_| "UKR".into());
    // A strike aircraft rolling in from 3,000 ft, two miles out; a wingman
    // low beside the target; straight down from overhead.
    let high = [-6_000., 3_000., -9_000.];
    let low = [-2_400., 250., -3_200.];
    let overhead = [0., 4_500., -400.];
    let mut shots = Vec::new();
    for (view, eye, fov) in [
        ("high", high, 12.),
        ("low", low, 30.),
        ("overhead", overhead, 30.),
    ] {
        for seconds in [0.1, 0.35, 0.7, 1.2, 1.7] {
            shots.push(Shot {
                name: view,
                kind: 35,
                seconds,
                eye,
                fov,
                crater: true,
            });
        }
    }
    for seconds in [0.35, 1.2] {
        shots.push(Shot {
            name: "agm-low",
            kind: 21,
            seconds,
            eye: low,
            fov: 30.,
            crater: true,
        });
        shots.push(Shot {
            name: "water-low",
            kind: 34,
            seconds,
            eye: low,
            fov: 30.,
            crater: false,
        });
    }
    for (light, condition, time) in [("day", None, Some([13, 0])), ("dusk", Some(4), None)] {
        let overrides = Overrides {
            time,
            wind: None,
            cloud_altitude: Some(0),
            redrawn_airports: false,
        };
        let world = Terrain::for_mission(resources, &theater, condition, &overrides)?;
        let mut scenery = Scenery::build(resources, &world)?;
        let mut gpu = pollster::block_on(Gpu::new(&scenery))?;
        let art = CombatArt::load(resources)?;
        let ownship = crate::aircraft::Airframe::load(resources, AircraftId::F18)?;
        let start = ownship.start(&world);
        // The blast stands on the ground ahead of the theater's start, or on
        // the sea at sea level for the water type.
        let land = [start.position[0] + 20_000., 0., start.position[2] + 20_000.];
        // The nearest open sea to the start, searched on a 4,000 ft grid, with
        // water all round the camera's side of it.
        let sea = (1..200_i32)
            .flat_map(|ring| {
                (-ring..=ring).flat_map(move |a| [(a, -ring), (a, ring), (-ring, a), (ring, a)])
            })
            .map(|(east, north)| {
                [
                    start.position[0] + f64::from(east) * 4_000.,
                    0.,
                    start.position[2] + f64::from(north) * 4_000.,
                ]
            })
            .find(|p| {
                [
                    [0., 0.],
                    [-2_400., -3_200.],
                    [-1_200., -1_600.],
                    [600., 600.],
                ]
                .iter()
                .all(|d| world.over_water(p[0] + d[0], p[2] + d[1]))
            });
        for shot in &shots {
            if light == "dusk" && shot.name != "low" {
                continue;
            }
            let mut point = if shot.kind == 34 {
                let Some(sea) = sea else {
                    println!("Blast preview: no open sea near the {theater} start");
                    continue;
                };
                sea
            } else {
                land
            };
            point[1] = f64::from(world.height(point[0] as f32, point[2] as f32)).max(0.);
            let elapsed = (shot.seconds * 120.) as u16;
            let effects = [EffectPose {
                kind: if shot.kind == 34 {
                    EffectKind::Ground
                } else {
                    EffectKind::Destroyed
                },
                position: point,
                ticks: LIFE - elapsed.min(LIFE - 1),
                blast: Some(shot.kind),
            }];
            let marks: Vec<MarkPose> = if shot.crater {
                vec![MarkPose {
                    kind: MarkKind::Crater(18),
                    position: point,
                    age: u64::from(elapsed),
                    strength: 1.,
                }]
            } else {
                Vec::new()
            };
            let mut camera = Camera::new();
            camera.position = std::array::from_fn(|i| point[i] + shot.eye[i]);
            let ground =
                f64::from(world.height(camera.position[0] as f32, camera.position[2] as f32));
            camera.position[1] = camera.position[1].max(ground + 60.);
            let d: [f64; 3] = std::array::from_fn(|i| point[i] - camera.position[i]);
            camera.yaw = d[0].atan2(d[2]) as f32;
            camera.pitch = d[1].atan2(d[0].hypot(d[2])) as f32;
            camera.zoom = (30_f64.to_radians().tan() / (shot.fov / 2.).to_radians().tan()) as f32;
            scenery.resolve_palette(&world, camera.position[1]);
            scenery.set_origin(camera.position);
            let device = &gpu.device;
            let queue = &gpu.queue;
            let devices = tore_sim::combat::countermeasures::Devices::default();
            let empty = tore_sim::combat::smoke::Smoke::default();
            gpu.sim.vapor(device, queue, &[]);
            gpu.sim
                .smoke(device, queue, &art.smoke, [&empty, &empty], &devices);
            gpu.sim
                .effects(device, queue, &art.effects, &effects, &marks);
            gpu.sim
                .emitters(queue, &devices, &[], &crate::gun_flash::Drawn::default());
            gpu.sim.combat(
                device,
                queue,
                &crate::render_snapshot::combat_geometry(
                    &RenderSnapshot::default(),
                    &art,
                    &ownship,
                    &start,
                    &camera,
                    &world,
                    &scenery,
                ),
            );
            gpu.sim.aircraft(device, queue, &ownship, &[]);
            let pixels = gpu.pixels(&camera, &world, &scenery)?;
            let path = out.join(format!(
                "{}-type{}-t{:.2}s-{light}.png",
                shot.name, shot.kind, shot.seconds
            ));
            std::fs::write(
                &path,
                crate::replay::png::encode_rgba(WIDTH, HEIGHT, &pixels)?,
            )?;
            println!("Blast preview: {}", path.display());
        }
    }
    Ok(())
}
