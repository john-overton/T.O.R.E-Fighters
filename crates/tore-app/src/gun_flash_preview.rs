//! `--gun-flash-preview OUT_DIR`: offscreen renders of the AC-130's muzzle
//! flashes, gun light, blast smoke and the 105 mm tracer, in daylight and at
//! night, from outside and through the gunsight camera. A development aid
//! for docs/spec/ac130-linked-guns.md#muzzle-flash: no window opens, nothing
//! is simulated, the shots are placed by hand on a posed aircraft.
use crate::{
    AppResult,
    camera::Camera,
    gun_flash::{Mount, Tracker},
    reel::{Gpu, HEIGHT, WIDTH},
    render_snapshot::CombatArt,
    scenery::Scenery,
    snapshot::{ProjectilePose, RenderSnapshot},
    terrain::{Overrides, Terrain},
};
use std::path::Path;
use tore_formats::aircraft::AircraftId;
use tore_sim::{attitude::Basis, combat::countermeasures::Devices};

/// Gunship speed for the moving 105 mm sequence: 250 knots in feet a second.
const SPEED_FPS: f64 = 422.;
/// Feet above the ground the aircraft is posed at.
const HEIGHT_FT: f64 = 3000.;

/// One picture: what fires, how long ago, and where the camera is.
struct Shot<'a> {
    name: &'a str,
    /// (gun slot, ticks before the picture) for each shot.
    fired: &'a [(usize, f64)],
    /// Ticks the aircraft has flown since the first shot.
    flown: f64,
    view: View,
    tracers: bool,
}
#[derive(Clone, Copy)]
enum View {
    /// Eye and look point in the aircraft's right, up, forward feet, and the
    /// vertical field of view in degrees.
    Outside([f64; 3], [f64; 3], f64),
    /// The same, but placed where the aircraft was at the first shot and
    /// left there, so what stays in the air is seen falling behind.
    Fixed([f64; 3], [f64; 3], f64),
    /// The gunsight camera from sensor dome D at a body-relative look
    /// (degrees) and zoom step; the own airframe is not drawn, as on the page.
    Sight([f64; 2], u8),
}

pub fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let [out] = args.as_slice() else {
        return Err("--gun-flash-preview OUTPUT_DIRECTORY".into());
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    let theater = std::env::var("TORE_PREVIEW_THEATER").unwrap_or_else(|_| "UKR".into());
    let side: View = View::Outside([-95., -22., 35.], [-12., -4., -2.], 40.);
    let far: View = View::Outside([-520., -160., 260.], [-10., 0., 0.], 30.);
    let aft: View = View::Outside([-70., -12., -110.], [-14., -6., 0.], 45.);
    let behind: View = View::Fixed([-260., -40., -120.], [-40., -10., 120.], 50.);
    let across: View = View::Outside([-110., 5., -5.], [-300., -135., 0.], 40.);
    // The 25 mm firing a burst: one round every 4 ticks for the last third
    // of a second (1,800 rounds a minute), so the flash is in mid-flicker.
    let burst: Vec<(usize, f64)> = (0..10).map(|n| (0, 1. + 4. * n as f64)).collect();
    let all: Vec<(usize, f64)> = burst.iter().copied().chain([(1, 2.), (2, 1.)]).collect();
    let pictures = [
        Shot {
            name: "outside-25mm",
            fired: &burst,
            flown: 0.,
            view: side,
            tracers: false,
        },
        Shot {
            name: "outside-40mm",
            fired: &[(1, 1.)],
            flown: 0.,
            view: side,
            tracers: false,
        },
        Shot {
            name: "outside-105mm",
            fired: &[(2, 1.)],
            flown: 0.,
            view: side,
            tracers: false,
        },
        Shot {
            name: "outside-all-guns",
            fired: &all,
            flown: 0.,
            view: side,
            tracers: false,
        },
        Shot {
            name: "far-all-guns",
            fired: &all,
            flown: 0.,
            view: far,
            tracers: false,
        },
        Shot {
            name: "105mm-t0.5",
            fired: &[(2, 0.5)],
            flown: 0.5,
            view: aft,
            tracers: false,
        },
        Shot {
            name: "105mm-t4",
            fired: &[(2, 4.)],
            flown: 4.,
            view: aft,
            tracers: false,
        },
        Shot {
            name: "105mm-t10",
            fired: &[(2, 10.)],
            flown: 10.,
            view: aft,
            tracers: false,
        },
        Shot {
            name: "105mm-t30",
            fired: &[(2, 30.)],
            flown: 30.,
            view: behind,
            tracers: false,
        },
        Shot {
            name: "105mm-t90",
            fired: &[(2, 90.)],
            flown: 90.,
            view: behind,
            tracers: false,
        },
        Shot {
            name: "105mm-t180",
            fired: &[(2, 180.)],
            flown: 180.,
            view: behind,
            tracers: false,
        },
        Shot {
            name: "tracers-25mm-vs-105mm",
            fired: &[],
            flown: 0.,
            view: across,
            tracers: true,
        },
        Shot {
            name: "sight-default-105mm",
            fired: &[(2, 1.)],
            flown: 0.,
            view: View::Sight([-90., -25.], 1),
            tracers: false,
        },
        Shot {
            name: "sight-aft-105mm",
            fired: &[(2, 1.)],
            flown: 0.,
            view: View::Sight([-150., -10.], 1),
            tracers: false,
        },
        Shot {
            name: "sight-aft-105mm-t8",
            fired: &[(2, 8.)],
            flown: 8.,
            view: View::Sight([-150., -10.], 1),
            tracers: false,
        },
    ];
    for (light, hour) in [("day", 13), ("dusk", 19), ("night", 23)] {
        let overrides = Overrides {
            time: Some([hour, 0]),
            wind: None,
            cloud_altitude: Some(0),
        };
        let world = Terrain::for_mission(resources, &theater, None, &overrides)?;
        let mut scenery = Scenery::build(resources, &world)?;
        let mut gpu = pollster::block_on(Gpu::new(&scenery))?;
        let art = CombatArt::load(resources)?;
        let ownship = crate::aircraft::Airframe::load(resources, AircraftId::Ac130)?;
        let mut start = ownship.start(&world);
        let ground = f64::from(world.height(start.position[0] as f32, start.position[2] as f32));
        start.position[1] = ground.max(0.) + HEIGHT_FT;
        start.gear = 0.;
        start.flaps = 0.;
        start.speed = SPEED_FPS;
        start.yaw = 0.;
        start.pitch = 0.;
        // Banked into a left orbit: the left wing down.
        start.bank = if Basis::new(0., 0., 0.3).right[1] > 0. {
            0.3
        } else {
            -0.3
        };
        // Every gun trained abeam left and 25 degrees down, on the sight.
        start.gun_aim = [[-0.5, -25. / 90.]; 3];
        for picture in &pictures {
            if picture.name.starts_with("105mm-t") && light == "dusk" {
                continue;
            }
            let mut state = start.clone();
            let basis = Basis::new(state.yaw, state.pitch, state.bank);
            for i in 0..3 {
                state.position[i] += basis.forward[i] * SPEED_FPS * picture.flown / 120.;
            }
            state.velocity = basis.forward.map(|v| v * SPEED_FPS);
            let now = picture.flown;
            let mut tracker = Tracker::default();
            let mut ordered = picture.fired.to_vec();
            ordered.sort_by(|a, b| b.1.total_cmp(&a.1));
            for (slot, before) in ordered {
                let at = now - before;
                // The aircraft as it was when the round left.
                let mut then = start.clone();
                for i in 0..3 {
                    then.position[i] += basis.forward[i] * SPEED_FPS * at / 120.;
                }
                tracker.fire(0, slot, at, &Mount::of_state(&then));
            }
            let mount = Mount::of_state(&state);
            let guns = tracker.draw(now, |_| Some(mount));
            let (camera, own_drawn) = camera(picture.view, &state, &start, &basis);
            scenery.resolve_palette(&world, camera.position[1]);
            scenery.set_origin(camera.position);
            let device = &gpu.device;
            let queue = &gpu.queue;
            let devices = Devices::default();
            let empty = tore_sim::combat::smoke::Smoke::default();
            gpu.sim.vapor(device, queue, &[]);
            gpu.sim
                .smoke(device, queue, &art.smoke, [&empty, &empty], &devices);
            gpu.sim.effects(device, queue, &art.effects, &[], &[]);
            gpu.sim.emitters(queue, &devices, &[], &guns);
            let snapshot = RenderSnapshot {
                projectiles: if picture.tracers {
                    tracers(&mount)
                } else {
                    Vec::new()
                },
                ..RenderSnapshot::default()
            };
            gpu.sim.combat(
                device,
                queue,
                &crate::render_snapshot::combat_geometry(
                    &snapshot, &art, &ownship, &state, &camera, &world, &scenery,
                ),
            );
            let vertices = if own_drawn {
                ownship.vertices(&state, &camera, &world, &scenery)
            } else {
                Vec::new()
            };
            gpu.sim.aircraft(device, queue, &ownship, &vertices);
            let pixels = gpu.pixels(&camera, &world, &scenery)?;
            let path = out.join(format!("{}-{light}.png", picture.name));
            std::fs::write(
                &path,
                crate::replay::png::encode_rgba(WIDTH, HEIGHT, &pixels)?,
            )?;
            println!(
                "Gun flash preview: {} ({} flashes, {} lights, {} puffs)",
                path.display(),
                guns.flashes.len(),
                guns.lights.len(),
                guns.puffs.len()
            );
        }
    }
    Ok(())
}

fn camera(
    view: View,
    state: &crate::flight::State,
    start: &crate::flight::State,
    basis: &Basis,
) -> (Camera, bool) {
    match view {
        View::Outside(eye, look, fov) | View::Fixed(eye, look, fov) => {
            let base = if matches!(view, View::Fixed(..)) {
                start.position
            } else {
                state.position
            };
            let place = |p: [f64; 3]| -> [f64; 3] {
                std::array::from_fn(|i| {
                    base[i] + basis.right[i] * p[0] + basis.up[i] * p[1] + basis.forward[i] * p[2]
                })
            };
            let mut camera = Camera::new();
            camera.position = place(eye);
            let target = place(look);
            let d: [f64; 3] = std::array::from_fn(|i| target[i] - camera.position[i]);
            camera.yaw = d[0].atan2(d[2]) as f32;
            camera.pitch = d[1].atan2(d[0].hypot(d[2])) as f32;
            camera.zoom = (30_f64.to_radians().tan() / (fov / 2.).to_radians().tan()) as f32;
            (camera, true)
        }
        View::Sight(look, step) => {
            let launcher = crate::combat::launcher(state);
            let view = crate::gunsight_view::free_view(
                &launcher,
                [look[0].to_radians(), look[1].to_radians()],
                step,
            );
            (crate::gunsight_view::camera(&view), false)
        }
    }
}

/// A 25 mm and a 105 mm tracer side by side, out along the guns' line: one
/// tick of each round's flight at its muzzle speed.
fn tracers(mount: &Mount) -> Vec<ProjectilePose> {
    [(0_usize, 3450., 1), (2, 1620., 2)]
        .into_iter()
        .map(|(slot, speed, id)| {
            let (muzzle, direction) = mount.muzzle(slot);
            let out = if slot == 0 { 260. } else { 320. };
            let position: [f64; 3] = std::array::from_fn(|i| muzzle[i] + direction[i] * out);
            ProjectilePose {
                id,
                owner: 0,
                weapon: tore_sim::combat::gunship::GUNS[slot].into(),
                shape: None,
                gun: true,
                tracer: true,
                position,
                previous: std::array::from_fn(|i| position[i] - direction[i] * speed / 120.),
                direction,
                target: None,
                incoming: false,
                speed_f8: (speed * 256.) as i32,
            }
        })
        .collect()
}
