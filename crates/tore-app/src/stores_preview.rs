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
const AIRCRAFT: [(AircraftId, &str); 12] = [
    (AircraftId::F18, "f18d"),
    (AircraftId::F16C, "f16c"),
    (AircraftId::F14, "f14"),
    (AircraftId::A10, "a10"),
    (AircraftId::Su25, "su25"),
    (AircraftId::Mig29, "mig29"),
    (AircraftId::Av8, "av8"),
    (AircraftId::A4E, "a4e"),
    (AircraftId::F15, "f15"),
    (AircraftId::F4J, "f4j"),
    (AircraftId::F4E, "f4e"),
    (AircraftId::Rafale, "rafale"),
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
        // `TORE_STORES_PREVIEW_TIME=HH:MM` poses them at another hour (a
        // night or dusk check of the stores' lighting).
        time: Some(
            std::env::var("TORE_STORES_PREVIEW_TIME")
                .ok()
                .and_then(|t| {
                    let (h, m) = t.split_once(':')?;
                    Some([h.parse().ok()?, m.parse().ok()?])
                })
                .unwrap_or([13, 0]),
        ),
        wind: None,
        cloud_altitude: Some(0),
        redrawn_airports: crate::scenery::redrawn_airports()?,
        city_fill: crate::scenery::city_fill()?,
        city_fill_cap: crate::scenery::city_fill_cap()?,
    };
    let world = Terrain::for_mission(resources, "UKR", None, &overrides)?;
    let mut scenery = Scenery::build(resources, &world)?;
    let mut gpu = pollster::block_on(Gpu::new(&scenery))?;
    // `TORE_STORES_PREVIEW_AIRCRAFT=f14,f15` draws only those.
    let only: Option<Vec<String>> = std::env::var("TORE_STORES_PREVIEW_AIRCRAFT")
        .ok()
        .map(|list| list.split(',').map(str::to_owned).collect());
    for (id, short) in AIRCRAFT
        .into_iter()
        .filter(|(_, short)| only.as_ref().is_none_or(|o| o.iter().any(|s| s == short)))
    {
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
        // Each loaded external station: the record, where its stores hang
        // and how deep the drawn model's underside is over each store's lugs.
        for (index, s) in config.stations.iter().enumerate() {
            if !s.rack.location().external() || live::is_gun(&s.weapon) || s.count == 0 {
                continue;
            }
            let size = s.size();
            let points: Vec<String> = (0..s.count)
                .map(|n| {
                    let p = s.release_point(u32::from(n));
                    let lugs = p[1] + size.top;
                    let depth = airframe
                        .underside(p[0], p[2], lugs, 6)
                        .map(|u| format!("{:.2}", u - lugs));
                    format!(
                        "[{:.2} {:.2} {:.2}] depth {}",
                        p[0],
                        p[1],
                        p[2],
                        depth.as_deref().unwrap_or("rail")
                    )
                })
                .collect();
            println!(
                "Stores preview: {short} station {index} {} x{} {:?} record {:.2?} over {:?}: {}",
                s.weapon.source,
                s.count,
                s.rack.location(),
                s.mount,
                s.rack.over,
                points.join("; ")
            );
        }
        for t in &config.tanks {
            println!(
                "Stores preview: {short} tank hardpoint {} x{} record {:.2?} over {:?}",
                t.hardpoint, t.quantity, t.mount, t.rack.over
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
        let close_only = std::env::var_os("TORE_STORES_PREVIEW_CLOSE").is_some();
        for (load, stores) in &loads {
            for (view, eye, look, fov) in VIEWS {
                if close_only {
                    continue;
                }
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
        // Close-ups of the loaded aircraft's joins: the outermost station
        // (a wingtip rail where there is one) and the busiest wing station,
        // from below and ahead, and from behind and below.
        let external: Vec<_> = config
            .stations
            .iter()
            .filter(|s| s.rack.location().external() && !live::is_gun(&s.weapon) && s.count > 0)
            .collect();
        let tip = external
            .iter()
            .map(|s| s.release_point(0))
            .max_by(|a, b| a[0].abs().total_cmp(&b[0].abs()));
        let wing = external
            .iter()
            .filter(|s| s.rack.location() == tore_sim::combat::hardpoints::Location::Wing)
            .max_by_key(|s| s.count)
            .map(|s| s.release_point(0));
        let fuselage = external
            .iter()
            .filter(|s| {
                matches!(
                    s.rack.location(),
                    tore_sim::combat::hardpoints::Location::Fuselage
                        | tore_sim::combat::hardpoints::Location::Centerline
                )
            })
            .max_by_key(|s| s.count)
            .map(|s| s.release_point(0));
        let close = [("tip", tip), ("wing", wing), ("fuselage", fuselage)];
        for (name, target) in close {
            let Some(t) = target else { continue };
            let t = [t[0].abs(), t[1], t[2]];
            for (view, eye) in [
                ("front", [t[0] + 7., t[1] - 6., t[2] + 22.]),
                ("rear", [t[0] + 5., t[1] - 7., t[2] - 22.]),
                ("ahead", [t[0] + 0.5, t[1] - 0.5, t[2] + 30.]),
                ("side", [t[0] + 18., t[1] - 5., t[2]]),
            ] {
                render(
                    &mut gpu,
                    &mut scenery,
                    &world,
                    &art,
                    &airframe,
                    &state,
                    &loads[1].1,
                    &[],
                    (eye, t, 34.),
                    &out.join(format!("{short}-close-{name}-{view}.png")),
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
        for station in order.into_iter().filter(|_| !close_only) {
            while rounds[station] > 0 && frame < 12 {
                let s = &config.stations[station];
                let debit = u32::from(s.weapon.burst.actual_rounds_per_game).max(1);
                rounds[station] = rounds[station].saturating_sub(debit);
                let units = tore_sim::combat::hardpoints::units(rounds[station], debit);
                let mount = s.release_point(units);
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

/// `--stores-sizes`: measurement table of every weapon shape (length, body
/// width, fin span, height, and the shape's header scale) and every
/// registered aircraft's drawn length, span and height, feet, as drawn.
pub fn sizes() -> AppResult<()> {
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    println!("weapon\tname\tshape\texp_scale\tlength\twidth\theight\ttop\tmiddle\tbody(20..80%)");
    for (key, bytes) in resources {
        if !key.ends_with(".JT") {
            continue;
        }
        let Ok(w) = tore_formats::weapons::Weapon::parse(key, bytes) else {
            continue;
        };
        let Some(shape_name) = w.shape.as_deref() else {
            continue;
        };
        let Some(sb) = resources.get(shape_name) else {
            continue;
        };
        let Ok(shape) = tore_formats::shape::Shape::parse(sb) else {
            continue;
        };
        let exp = tore_formats::shape::object_scale(sb).unwrap_or(0.);
        if let Some(real) = crate::store_sizes::real_size(shape_name) {
            let fitted = crate::store_sizes::fitted(&shape, &real, real.span);
            let b = crate::store_sizes::body(&shape);
            let f = crate::store_sizes::body(&fitted);
            println!("fit\t{key}\t{shape_name}\tretail {b:.2?}\tfitted {f:.2?}");
        }
        let [low, high] = bounds(
            shape
                .faces
                .iter()
                .filter(|f| !matches!(f.subtype, 0x4c | 0x5c | 0x6c | 0x7c)),
        );
        let body: Vec<String> = [0.2, 0.35, 0.5, 0.65, 0.8]
            .iter()
            .map(|t| {
                let c = low[1] + t * (high[1] - low[1]);
                let [w, h] = section(&shape, c);
                format!("{w:.2}x{h:.2}")
            })
            .collect();
        println!(
            "{key}\t{}\t{shape_name}\t{exp}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{}",
            w.name,
            high[1] - low[1],
            high[0] - low[0],
            high[2] - low[2],
            high[2],
            (high[1] + low[1]) / 2.,
            body.join(" ")
        );
    }
    println!("aircraft\tscale\tlength\tspan\theight");
    for id in AircraftId::ALL {
        let Ok(airframe) = crate::aircraft::Airframe::load(resources, id) else {
            continue;
        };
        let scale = f64::from(airframe.animation_scale());
        let Some(shape) = airframe.poses.first() else {
            continue;
        };
        let [low, high] = bounds(
            shape
                .faces
                .iter()
                .filter(|f| !matches!(f.subtype, 0x4c | 0x5c | 0x6c | 0x7c)),
        );
        let s = 3. * scale;
        println!(
            "{id:?}\t{scale:.4}\t{:.2}\t{:.2}\t{:.2}",
            (high[1] - low[1]) * s,
            (high[0] - low[0]) * s,
            (high[2] - low[2]) * s
        );
    }
    Ok(())
}

/// Lowest and highest corner of `faces`, feet at a third of a foot a unit.
fn bounds<'a>(faces: impl Iterator<Item = &'a tore_formats::shape::Face>) -> [[f64; 3]; 2] {
    let mut low = [f64::MAX; 3];
    let mut high = [f64::MIN; 3];
    for q in faces.flat_map(|f| f.positions.iter()) {
        for k in 0..3 {
            low[k] = low[k].min(f64::from(q[k]) / 3.);
            high[k] = high[k].max(f64::from(q[k]) / 3.);
        }
    }
    [low, high]
}

/// Width and height, feet, of `shape`'s cross-section at `forward` feet.
pub(crate) fn section(shape: &tore_formats::shape::Shape, forward: f64) -> [f64; 2] {
    let mut low = [f64::MAX; 2];
    let mut high = [f64::MIN; 2];
    for f in shape
        .faces
        .iter()
        .filter(|f| !matches!(f.subtype, 0x4c | 0x5c | 0x6c | 0x7c))
    {
        let n = f.positions.len();
        for i in 0..n {
            let a = f.positions[i].map(|v| f64::from(v) / 3.);
            let b = f.positions[(i + 1) % n].map(|v| f64::from(v) / 3.);
            if (a[1] - forward) * (b[1] - forward) > 0. || (a[1] - b[1]).abs() < 1e-9 {
                continue;
            }
            let t = (forward - a[1]) / (b[1] - a[1]);
            let p = [a[0] + t * (b[0] - a[0]), a[2] + t * (b[2] - a[2])];
            for k in 0..2 {
                low[k] = low[k].min(p[k]);
                high[k] = high[k].max(p[k]);
            }
        }
    }
    if low[0] > high[0] {
        return [0.; 2];
    }
    [high[0] - low[0], high[1] - low[1]]
}

/// `--stores-size-sheet OUT_DIR`: side and top silhouettes of every weapon
/// shape with a listed real size, retail (dark) under refitted (light), on a
/// one-foot grid with a ten-foot ruler, plus the fitted rocket pods; two
/// sheets, missiles and bombs.
pub fn size_sheet() -> AppResult<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let [out] = args.as_slice() else {
        return Err("--stores-size-sheet OUTPUT_DIRECTORY".into());
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    let font =
        tore_formats::font::Font::parse(resources.get("WIN11.FNT").ok_or("missing WIN11.FNT")?)?;
    // Weapon names by shape, for the labels.
    let mut names: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for (key, bytes) in resources {
        if let Some(shape) = key
            .ends_with(".JT")
            .then(|| tore_formats::weapons::Weapon::parse(key, bytes).ok())
            .flatten()
            .and_then(|w| w.shape.clone().map(|s| (s, w.name)))
        {
            let list = names.entry(shape.0.to_ascii_uppercase()).or_default();
            if !list.contains(&shape.1) {
                list.push(shape.1);
            }
        }
    }
    let split = crate::store_sizes::SIZES
        .iter()
        .position(|(name, _)| *name == "MK82.SH")
        .unwrap_or(0);
    for (sheet, rows) in [
        (
            "stores-sizes-missiles.png",
            &crate::store_sizes::SIZES[..split],
        ),
        (
            "stores-sizes-bombs.png",
            &crate::store_sizes::SIZES[split..],
        ),
    ] {
        const PX: f64 = 26.;
        let column = (22. * PX) as usize;
        let width = 40 + 2 * column + 40;
        let heights: Vec<usize> = rows
            .iter()
            .map(|(_, real)| 34 + (real.span.max(5.) * PX) as usize)
            .collect();
        let height = 70 + heights.iter().sum::<usize>();
        let mut canvas = Sheet::new(width, height);
        canvas.text(
            &font,
            "SIDE VIEW (RETAIL DARK, REAL SIZE LIGHT)",
            40,
            8,
            [240; 3],
        );
        canvas.text(&font, "TOP VIEW", 60 + column as i64, 8, [240; 3]);
        // A ten-foot ruler.
        let ruler = (10. * PX) as i64;
        canvas.rect(40, 44, ruler, 3, [250, 220, 90]);
        for foot in 0..=10 {
            canvas.rect(40 + (f64::from(foot) * PX) as i64, 38, 2, 9, [250, 220, 90]);
        }
        canvas.text(&font, "10 FT", 50 + ruler, 34, [250, 220, 90]);
        let mut y0 = 70;
        for ((name, real), h) in rows.iter().zip(&heights) {
            let Some(shape) = resources
                .get(*name)
                .and_then(|bytes| tore_formats::shape::Shape::parse(bytes).ok())
            else {
                y0 += h;
                continue;
            };
            let fitted = crate::store_sizes::fitted(&shape, real, real.span);
            let label = format!(
                "{} {}  REAL {:.1} FT LONG, {:.2} BODY, {:.1} FINS",
                name.trim_end_matches(".SH"),
                names.get(*name).map(|n| n.join("/")).unwrap_or_default(),
                real.length,
                real.diameter,
                real.span
            );
            canvas.text(&font, &label, 40, y0 as i64, [235; 3]);
            let mid = y0 as f64 + 22. + real.span.max(5.) * PX / 2.;
            // A faint one-foot grid behind.
            for foot in 0..=21 {
                for x0 in [40, 40 + column + 20] {
                    let x = x0 as i64 + (f64::from(foot) * PX) as i64;
                    canvas.rect(x, (y0 + 22) as i64, 1, (h - 30) as i64, [70, 80, 92]);
                }
            }
            for (view, x0) in [(2usize, 40.), (0usize, 40. + column as f64 + 20.)] {
                for (s, color) in [(&shape, [40, 46, 54]), (&fitted, [170, 205, 240])] {
                    let along = crate::store_sizes::body(s).map_or(0., |b| b.along[0]);
                    for face in s
                        .faces
                        .iter()
                        .filter(|f| !crate::store_sizes::texture_only(f))
                    {
                        let p: Vec<[f64; 2]> = face
                            .positions
                            .iter()
                            .map(|q| {
                                [
                                    x0 + (f64::from(q[1]) / 3. - along) * PX,
                                    mid - f64::from(q[view]) / 3. * PX,
                                ]
                            })
                            .collect();
                        for i in 1..p.len().saturating_sub(1) {
                            canvas.triangle([p[0], p[i], p[i + 1]], color);
                        }
                    }
                }
            }
            y0 += h;
        }
        let path = out.join(sheet);
        canvas.png(&path)?;
        println!("Stores size sheet: {}", path.display());
    }
    Ok(())
}

/// A plain RGB picture for the size sheets.
struct Sheet {
    width: usize,
    height: usize,
    rgb: Vec<[u8; 3]>,
}
impl Sheet {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            rgb: vec![[52, 60, 70]; width * height],
        }
    }
    fn put(&mut self, x: i64, y: i64, color: [u8; 3]) {
        if (0..self.width as i64).contains(&x) && (0..self.height as i64).contains(&y) {
            self.rgb[y as usize * self.width + x as usize] = color;
        }
    }
    fn rect(&mut self, x: i64, y: i64, w: i64, h: i64, color: [u8; 3]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, color);
            }
        }
    }
    fn text(
        &mut self,
        font: &tore_formats::font::Font,
        text: &str,
        x: i64,
        y: i64,
        color: [u8; 3],
    ) {
        let mut pen = x;
        for byte in text.bytes() {
            let Some(glyph) = font.glyphs.get(usize::from(byte)) else {
                continue;
            };
            for &(gx, gy) in &glyph.pixels {
                self.rect(pen + 2 * gx as i64, y + 2 * gy as i64, 2, 2, color);
            }
            pen += 2 * glyph.advance as i64;
        }
    }
    fn triangle(&mut self, p: [[f64; 2]; 3], color: [u8; 3]) {
        let lo = [0, 1].map(|k| p.iter().map(|q| q[k]).fold(f64::MAX, f64::min).floor() as i64);
        let hi = [0, 1].map(|k| p.iter().map(|q| q[k]).fold(f64::MIN, f64::max).ceil() as i64);
        let edge = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
            (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
        };
        let area = edge(p[0], p[1], p[2]);
        if area.abs() < 1e-9 {
            // Edge-on: draw its outline so thin fins still show.
            for (a, b) in [(p[0], p[1]), (p[1], p[2]), (p[2], p[0])] {
                let n = ((b[0] - a[0]).abs().max((b[1] - a[1]).abs()).ceil() as usize).max(1);
                for i in 0..=n {
                    let t = i as f64 / n as f64;
                    self.put(
                        (a[0] + t * (b[0] - a[0])).round() as i64,
                        (a[1] + t * (b[1] - a[1])).round() as i64,
                        color,
                    );
                }
            }
            return;
        }
        for y in lo[1]..=hi[1] {
            for x in lo[0]..=hi[0] {
                let c = [x as f64 + 0.5, y as f64 + 0.5];
                let w = [
                    edge(p[1], p[2], c) / area,
                    edge(p[2], p[0], c) / area,
                    edge(p[0], p[1], c) / area,
                ];
                if w.iter().all(|v| *v >= -1e-9) {
                    self.put(x, y, color);
                }
            }
        }
    }
    fn png(&self, path: &Path) -> AppResult<()> {
        let rgba: Vec<u8> = self
            .rgb
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect();
        std::fs::write(
            path,
            crate::replay::png::encode_rgba(self.width as u32, self.height as u32, &rgba)?,
        )?;
        Ok(())
    }
}
