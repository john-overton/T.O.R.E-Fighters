//! `--stores-preview OUT_DIR`: offscreen renders of the pylons and stores
//! (docs/spec/stores-on-pylons.md, B3) on several aircraft with their
//! default loads: before (nothing drawn, as before B3), loaded, half spent
//! and empty, from outside; and a release sequence, each released store shown
//! a moment after it left its pylon. A development aid: no window opens,
//! nothing is simulated, the stores are placed from the loadout by hand.
use crate::{
    AppResult,
    camera::Camera,
    reel::{Gpu, HEIGHT, WIDTH},
    render_snapshot::CombatArt,
    scenery::Scenery,
    snapshot::{ProjectilePose, RenderSnapshot, StorePose, store_poses},
    terrain::{Overrides, Terrain},
};
use std::path::Path;
use tore_formats::aircraft::{Aircraft, AircraftId};
use tore_sim::{
    attitude::Basis,
    combat::{countermeasures::Devices, live},
};

/// Feet above the ground the aircraft is posed at.
const HEIGHT_FT: f64 = 4000.;

/// The aircraft drawn, with a short file name each.
const AIRCRAFT: [(AircraftId, &str); 8] = [
    (AircraftId::F18, "f18d"),
    (AircraftId::F16C, "f16c"),
    (AircraftId::F14, "f14"),
    (AircraftId::A10, "a10"),
    (AircraftId::Su25, "su25"),
    (AircraftId::Mig29, "mig29"),
    (AircraftId::Av8, "av8"),
    (AircraftId::A4E, "a4e"),
];

/// Camera eye and look point, feet right, up and forward of the aircraft,
/// and the vertical field of view in degrees.
const VIEWS: [(&str, [f64; 3], [f64; 3], f64); 3] = [
    ("front-low", [-38., -16., 62.], [0., -3., 0.], 40.),
    ("below-side", [55., -40., -20.], [0., -3., -2.], 45.),
    ("rear-high", [-30., 18., -75.], [0., -2., 0.], 40.),
];

pub fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let [out] = args.as_slice() else {
        return Err("--stores-preview OUTPUT_DIRECTORY".into());
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    let overrides = Overrides {
        time: Some([13, 0]),
        wind: None,
        cloud_altitude: Some(0),
        redrawn_airports: crate::scenery::redrawn_airports()?,
        city_fill: crate::scenery::city_fill()?,
        city_fill_cap: crate::scenery::city_fill_cap()?,
    };
    let world = Terrain::for_mission(resources, "UKR", None, &overrides)?;
    let mut scenery = Scenery::build(resources, &world)?;
    let mut gpu = pollster::block_on(Gpu::new(&scenery))?;
    for (id, short) in AIRCRAFT {
        let airframe = crate::aircraft::Airframe::load(resources, id)?;
        let profile = Aircraft::parse(resources.get(id.pt()).ok_or("missing aircraft record")?)?;
        let config = live::Configuration::from_source(&profile, |name| {
            resources
                .get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
        })?;
        for (index, h) in profile.hardpoints.iter().enumerate() {
            println!(
                "Stores preview: {short} hardpoint {index} at {:?} pylon top {:?}",
                h.position,
                airframe.pylon_top(index)
            );
        }
        let mut art = CombatArt::load(resources)?;
        art.add_weapon_shapes(&config, resources);
        let full = |index: usize| {
            config
                .stations
                .get(index)
                .map_or(0, |s| u32::from(s.count) * live::pod_rounds(&s.weapon))
        };
        let loaded = |index: usize| config.stations.get(index).is_some_and(|s| s.count > 0);
        let loads: [(&str, Vec<StorePose>); 4] = [
            ("before", Vec::new()),
            ("loaded", store_poses(&config, full, loaded)),
            (
                "half",
                store_poses(&config, |i| full(i).div_ceil(2), loaded),
            ),
            ("empty", store_poses(&config, |_| 0, loaded)),
        ];
        let mut state = airframe.start(&world);
        let ground = f64::from(world.height(state.position[0] as f32, state.position[2] as f32));
        state.position[1] = ground.max(0.) + HEIGHT_FT;
        state.gear = 0.;
        state.flaps = 0.;
        state.yaw = 0.;
        state.pitch = 0.;
        state.bank = 0.;
        for (load, stores) in &loads {
            for (view, eye, look, fov) in VIEWS {
                let name = format!("{short}-{load}-{view}.png");
                render(
                    &mut gpu,
                    &mut scenery,
                    &world,
                    &art,
                    &airframe,
                    &state,
                    stores,
                    &[],
                    (eye, look, fov),
                    &out.join(&name),
                )?;
            }
        }
        // The release sequence: each release takes one store from its
        // pylon (the side the rule picks), and that store is drawn a fifth
        // of a second after it left, 0.6 ft lower.
        let mut rounds: Vec<u32> = (0..config.stations.len()).map(full).collect();
        let order: Vec<usize> = (0..config.stations.len())
            .filter(|i| {
                let s = &config.stations[*i];
                s.rack.location().external() && !live::is_gun(&s.weapon) && s.count > 0
            })
            .collect();
        let mut frame = 0;
        for station in order {
            while rounds[station] > 0 && frame < 12 {
                let s = &config.stations[station];
                let debit = u32::from(s.weapon.burst.actual_rounds_per_game).max(1);
                rounds[station] = rounds[station].saturating_sub(debit);
                let units = tore_sim::combat::hardpoints::units(rounds[station], debit);
                let mount = s.rack.release_mount(s.mount, units);
                let basis = Basis::new(state.yaw, state.pitch, state.bank);
                let position: [f64; 3] = std::array::from_fn(|i| {
                    state.position[i]
                        + basis.right[i] * mount[0]
                        + basis.up[i] * (mount[1] - 0.6)
                        + basis.forward[i] * mount[2]
                });
                let flying = vec![ProjectilePose {
                    id: frame as u32 + 1,
                    owner: 0,
                    weapon: s.weapon.source.clone(),
                    shape: s.weapon.shape.clone(),
                    gun: false,
                    tracer: false,
                    position,
                    previous: position,
                    direction: basis.forward,
                    target: None,
                    incoming: false,
                    speed_f8: 0,
                    powered: false,
                }];
                let stores = store_poses(&config, |i| rounds[i], loaded);
                let (_, eye, look, fov) = VIEWS[0];
                render(
                    &mut gpu,
                    &mut scenery,
                    &world,
                    &art,
                    &airframe,
                    &state,
                    &stores,
                    &flying,
                    (eye, look, fov),
                    &out.join(format!("{short}-release-{frame:02}.png")),
                )?;
                frame += 1;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render(
    gpu: &mut Gpu,
    scenery: &mut Scenery,
    world: &Terrain,
    art: &CombatArt,
    airframe: &crate::aircraft::Airframe,
    state: &crate::flight::State,
    stores: &[StorePose],
    flying: &[ProjectilePose],
    (eye, look, fov): ([f64; 3], [f64; 3], f64),
    path: &Path,
) -> AppResult<()> {
    let basis = Basis::new(state.yaw, state.pitch, state.bank);
    let place = |p: [f64; 3]| -> [f64; 3] {
        std::array::from_fn(|i| {
            state.position[i] + basis.right[i] * p[0] + basis.up[i] * p[1] + basis.forward[i] * p[2]
        })
    };
    let mut camera = Camera::new();
    camera.position = place(eye);
    let target = place(look);
    let d: [f64; 3] = std::array::from_fn(|i| target[i] - camera.position[i]);
    camera.yaw = d[0].atan2(d[2]) as f32;
    camera.pitch = d[1].atan2(d[0].hypot(d[2])) as f32;
    camera.zoom = (30_f64.to_radians().tan() / (fov / 2.).to_radians().tan()) as f32;
    camera.near_clip = 1.;
    scenery.resolve_palette(world, camera.position[1]);
    scenery.set_origin(camera.position);
    let device = &gpu.device;
    let queue = &gpu.queue;
    let devices = Devices::default();
    let empty = tore_sim::combat::smoke::Smoke::default();
    gpu.sim.vapor(device, queue, &[]);
    gpu.sim
        .smoke(device, queue, &art.smoke, [&empty, &empty], &devices);
    gpu.sim.effects(device, queue, &art.effects, &[], &[]);
    let snapshot = RenderSnapshot {
        projectiles: flying.to_vec(),
        ..RenderSnapshot::default()
    };
    gpu.sim.combat(
        device,
        queue,
        &crate::render_snapshot::combat_geometry(
            &snapshot, art, airframe, state, &camera, world, scenery,
        ),
    );
    let mut vertices = airframe.vertices(state, &camera, world, scenery);
    vertices.extend(crate::pylons::for_flown(
        stores, state, airframe, art, scenery,
    ));
    gpu.sim.aircraft(device, queue, airframe, &vertices);
    let pixels = gpu.pixels(&camera, world, scenery)?;
    std::fs::write(
        path,
        crate::replay::png::encode_rgba(WIDTH, HEIGHT, &pixels)?,
    )?;
    println!("Stores preview: {}", path.display());
    Ok(())
}
