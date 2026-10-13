//! `--impact-preview OUT_DIR`: offscreen renders of weapon impacts side by
//! side, so their sizes, light and sparks can be compared (slice BF, bugs B8
//! to B11). A row of ground impacts stands on open ground ahead of the
//! theater's start, 20 mm, 105 mm, then 250 lb, 500 lb and 2,000 lb bombs
//! left to right, seen from low beside the row by day and at night, a few
//! moments into the explosions; then close views of a 20 mm and a 105 mm
//! hit, and an eight-round 20 mm burst; the row also 3 and 6 seconds on,
//! when only the smoke is left. With `shockwave` after the folder it renders
//! the climate set instead (slice B12, [`SHOCKWAVE_ROW`]). A development
//! aid: no
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
/// None once it has played out.
fn ground_hit(weapon: &Weapon, position: [f64; 3], elapsed: u16) -> Option<EffectPose> {
    let kind = weapon.effects.land_explosion;
    let row = blast::explosion(kind)?;
    let life = u16::from(row.seconds) * 120;
    if elapsed >= life {
        return None;
    }
    Some(EffectPose {
        kind: EffectKind::Ground,
        position,
        ticks: life - elapsed,
        blast: Some(kind),
        scale: tore_sim::combat::blast_scale::of(weapon),
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
    /// Each impact as it went off and the picture tick it did: the flash
    /// and smoke follow it from there.
    births: Vec<(u64, EffectPose)>,
    /// Craters and pock marks on the ground, and fires.
    marks: Vec<crate::snapshot::MarkPose>,
    /// Each fire's spot and the width it is drawn at (a unit's fire).
    fires: Vec<([f64; 3], f64)>,
    eye: [f64; 3],
    look: [f64; 3],
    fov: f64,
    tick: u64,
}

/// The shockwave set (slice B12, `--impact-preview OUT_DIR shockwave`): a
/// 500 lb bomb, a 2,000 lb bomb and an AGM-65G 1,300 ft apart, seen from
/// low beside the row 0.1, 0.5, 2, 10, 30 and 60 s after they went off, so
/// the ring, the vapor and the lingering dust show.
const SHOCKWAVE_ROW: [Impact; 3] = [
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
    Impact {
        label: "agm65g",
        record: "AGM65G.JT",
        weight: None,
    },
];
const SHOCKWAVE_SPACING: f64 = 1_300.;
const SHOCKWAVE_SECONDS: [f64; 6] = [0.1, 0.5, 2., 10., 30., 60.];

/// The night range set (nr-fx3, `--impact-preview OUT_DIR night-range`):
/// a 250 lb, a 500 lb and a 2,000 lb bomb [`NIGHT_SPACING`] apart, and
/// behind them a 24 ft and a 90 ft building fire and a 160 ft fuel fire, at
/// night, seen from 3,000 ft up (800 ft a mile past that) at
/// [`NIGHT_RANGES_NM`] from the row, 0.15 s
/// after the bombs went off (the flash) and 1 s after (the fireball).
const NIGHT_ROW: [Impact; 3] = [
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
const NIGHT_SPACING: f64 = 1_200.;
const NIGHT_FIRES_FT: [f64; 3] = [24., 90., 160.];
const NIGHT_RANGES_NM: [f64; 6] = [0.3, 2., 5., 10., 20., 30.];
const NIGHT_SECONDS: [f64; 2] = [0.15, 1.];

fn night_range_shots(
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
    world: &Terrain,
    base: [f64; 3],
) -> AppResult<Vec<Shot>> {
    let row: Vec<Weapon> = NIGHT_ROW
        .iter()
        .map(|impact| record(resources, impact.record, impact.weight))
        .collect::<AppResult<_>>()?;
    let ground = |x: f64, z: f64| f64::from(world.height(x as f32, z as f32)).max(0.);
    let spot = |n: f64, back: f64| {
        let x = base[0] + n * NIGHT_SPACING;
        let z = base[2] + back;
        [x, ground(x, z), z]
    };
    let middle = spot(1., 300.);
    let births: Vec<(u64, EffectPose)> = row
        .iter()
        .enumerate()
        .filter_map(|(n, weapon)| ground_hit(weapon, spot(n as f64, 0.), 0))
        .map(|e| (10_000, e))
        .collect();
    let fires: Vec<([f64; 3], f64)> = NIGHT_FIRES_FT
        .iter()
        .enumerate()
        .map(|(n, width)| (spot(n as f64, 700.), *width))
        .collect();
    let mut shots = Vec::new();
    for nm in NIGHT_RANGES_NM {
        for seconds in NIGHT_SECONDS {
            let elapsed = (seconds * 120.) as u16;
            let away = nm * 6_076.12;
            shots.push(Shot {
                name: format!("night-{nm:04.1}nm-t{seconds:.2}s"),
                effects: row
                    .iter()
                    .enumerate()
                    .filter_map(|(n, weapon)| ground_hit(weapon, spot(n as f64, 0.), elapsed))
                    .collect(),
                births: births.clone(),
                marks: fires
                    .iter()
                    .map(|(at, _)| crate::snapshot::MarkPose {
                        kind: blast::MarkKind::Fire,
                        position: *at,
                        age: 10_000 + u64::from(elapsed),
                        strength: 1.,
                        scale: 100,
                    })
                    .collect(),
                fires: fires.clone(),
                // A strike aircraft's height for the range, clear of the hills.
                eye: [
                    middle[0],
                    middle[1] + (800. * nm).max(3_000.),
                    middle[2] - away,
                ],
                look: [middle[0], middle[1], middle[2]],
                fov: if nm < 1. { 70. } else { 40. },
                tick: 10_000 + u64::from(elapsed),
            });
        }
    }
    Ok(shots)
}

fn shockwave_shots(
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
    world: &Terrain,
    base: [f64; 3],
    light: &str,
) -> AppResult<Vec<Shot>> {
    let row: Vec<Weapon> = SHOCKWAVE_ROW
        .iter()
        .map(|impact| record(resources, impact.record, impact.weight))
        .collect::<AppResult<_>>()?;
    let ground = |x: f64, z: f64| f64::from(world.height(x as f32, z as f32)).max(0.);
    let spot = |n: f64| {
        let x = base[0] + n * SHOCKWAVE_SPACING;
        [x, ground(x, base[2]), base[2]]
    };
    let middle = spot(1.);
    let births: Vec<(u64, EffectPose)> = row
        .iter()
        .enumerate()
        .filter_map(|(n, weapon)| ground_hit(weapon, spot(n as f64), 0))
        .map(|e| (10_000, e))
        .collect();
    Ok(SHOCKWAVE_SECONDS
        .iter()
        .map(|&seconds| {
            let elapsed = (seconds * 120.) as u16;
            Shot {
                name: format!("shock-t{seconds:05.1}s-{light}"),
                effects: row
                    .iter()
                    .enumerate()
                    .filter_map(|(n, weapon)| ground_hit(weapon, spot(n as f64), elapsed))
                    .collect(),
                births: births.clone(),
                marks: row
                    .iter()
                    .enumerate()
                    .filter_map(|(n, weapon)| {
                        let (size, scale) = tore_sim::combat::blast_scale::crater_of(weapon)?;
                        Some(crate::snapshot::MarkPose {
                            kind: blast::MarkKind::Crater(size),
                            position: spot(n as f64),
                            age: u64::from(elapsed),
                            strength: 1.,
                            scale,
                        })
                    })
                    .collect(),
                fires: Vec::new(),
                eye: [middle[0], middle[1] + 260., middle[2] - 2_400.],
                look: [middle[0], middle[1] + 150., middle[2]],
                fov: 75.,
                tick: 10_000 + u64::from(elapsed),
            }
        })
        .collect())
}

pub fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let (out, climate, night_range) = match args.as_slice() {
        [out] => (out, false, false),
        [out, set] if set == "shockwave" => (out, true, false),
        [out, set] if set == "night-range" => (out, true, true),
        _ => return Err("--impact-preview OUTPUT_DIRECTORY [shockwave|night-range]".into()),
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
        if night_range && light == "day" {
            continue;
        }
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
        if climate {
            // Open land for the whole row and the camera: the theater's
            // start may face the sea.
            let land = (0..200_i32)
                .flat_map(|ring| {
                    (-ring..=ring).flat_map(move |a| [(a, -ring), (a, ring), (-ring, a), (ring, a)])
                })
                .map(|(east, north)| {
                    [
                        base[0] + f64::from(east) * 4_000.,
                        0.,
                        base[2] + f64::from(north) * 4_000.,
                    ]
                })
                .find(|p| {
                    (-4..=12).all(|i| {
                        (-6..=2).all(|j| {
                            !world.drawn_as_water(
                                p[0] + f64::from(i) * 300.,
                                p[2] + f64::from(j) * 450.,
                            )
                        })
                    })
                })
                .ok_or("no open land near the theater's start")?;
            shots = if night_range {
                night_range_shots(resources, &world, land)?
            } else {
                shockwave_shots(resources, &world, land, light)?
            };
        } else {
            let row_births: Vec<(u64, EffectPose)> = row
                .iter()
                .enumerate()
                .filter_map(|(n, (_, weapon))| ground_hit(weapon, spot(n as f64), 0))
                .map(|e| (10_000, e))
                .collect();
            for seconds in [0.1, 0.3, 0.6, 1.0, 3.0, 6.0] {
                let elapsed = (seconds * 120.) as u16;
                let effects = row
                    .iter()
                    .enumerate()
                    .filter_map(|(n, (_, weapon))| ground_hit(weapon, spot(n as f64), elapsed))
                    .collect();
                shots.push(Shot {
                    name: format!("row-t{seconds:.2}s-{light}"),
                    effects,
                    marks: Vec::new(),
                    births: row_births.clone(),
                    fires: Vec::new(),
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
                        marks: Vec::new(),
                        births: ground_hit(weapon, at, 0)
                            .map(|e| (10_000, e))
                            .into_iter()
                            .collect(),
                        fires: Vec::new(),
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
            let place = |k: u16| {
                let along = f64::from(k) * 9.;
                [
                    at[0] + along,
                    ground(at[0] + along, at[2] + f64::from(k % 3) * 6.),
                    at[2] + f64::from(k % 3) * 6.,
                ]
            };
            for seconds in [0.1, 0.25] {
                let elapsed = (seconds * 120.) as u16;
                let effects = (0..8_u16)
                    .filter_map(|k| ground_hit(cannon, place(k), elapsed.saturating_sub(k * 2)))
                    .collect();
                shots.push(Shot {
                    name: format!("burst-20mm-t{seconds:.2}s-{light}"),
                    effects,
                    marks: Vec::new(),
                    births: (0..8_u16)
                        .filter_map(|k| {
                            ground_hit(cannon, place(k), 0).map(|e| (10_000 + u64::from(k) * 2, e))
                        })
                        .collect(),
                    fires: Vec::new(),
                    eye: [at[0] - 300., at[1] + 90., at[2] - 520.],
                    look: [at[0] + 30., at[1] + 10., at[2]],
                    fov: 30.,
                    tick: 10_000 + u64::from(elapsed),
                });
            }
            // Craters (slice BF, John 2026-10-12): the row's craters and a
            // strafing line of 40 20 mm rounds in front of it, as the base game
            // left them (each record's own crater size, guns none) and as now
            // (the explosion's scale, guns a pock mark), seen from above.
            let crater = |at: [f64; 3], (size, scale): (u8, u8)| crate::snapshot::MarkPose {
                kind: blast::MarkKind::Crater(size),
                position: at,
                age: 0,
                strength: 1.,
                scale,
            };
            for (label, now) in [("before", false), ("after", true)] {
                let mut marks: Vec<_> = row
                    .iter()
                    .enumerate()
                    .filter_map(|(n, (_, weapon))| {
                        let made = if now {
                            tore_sim::combat::blast_scale::crater_of(weapon)
                        } else {
                            (weapon.effects.crater_size > 0)
                                .then_some((weapon.effects.crater_size, 100))
                        };
                        made.map(|m| crater(spot(n as f64), m))
                    })
                    .collect();
                if now {
                    let (_, cannon) = &row[0];
                    if let Some(pock) = tore_sim::combat::blast_scale::crater_of(cannon) {
                        for k in 0..40 {
                            let x = base[0] - 400. + f64::from(k) * 12.;
                            let z = base[2] - 500. + f64::from(k % 3) * 3.;
                            marks.push(crater([x, ground(x, z), z], pock));
                        }
                    }
                }
                for (view, eye, look, fov) in [
                    (
                        "row",
                        [middle[0], middle[1] + 2_200., middle[2] - 2_600.],
                        [middle[0], middle[1], middle[2]],
                        40.,
                    ),
                    (
                        "strafe",
                        [
                            base[0] - 220.,
                            ground(base[0] - 160., base[2] - 500.) + 140.,
                            base[2] - 760.,
                        ],
                        [
                            base[0] - 160.,
                            ground(base[0] - 160., base[2] - 500.),
                            base[2] - 500.,
                        ],
                        40.,
                    ),
                ] {
                    shots.push(Shot {
                        name: format!("craters-{view}-{label}-{light}"),
                        effects: Vec::new(),
                        births: Vec::new(),
                        marks: marks.clone(),
                        fires: Vec::new(),
                        eye,
                        look,
                        fov,
                        tick: 20_000,
                    });
                }
            }
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
            // The light and smoke the impacts give, as a flight draws them:
            // the tracker sees each go off, then draws at the shot's time.
            let mut tracker = crate::gun_flash::Tracker::default();
            let mut ticks: Vec<u64> = shot.births.iter().map(|(t, _)| *t).collect();
            ticks.dedup();
            for tick in ticks {
                let born = RenderSnapshot {
                    tick,
                    effects: shot
                        .births
                        .iter()
                        .filter(|(t, _)| *t == tick)
                        .map(|(_, e)| *e)
                        .collect(),
                    ..RenderSnapshot::default()
                };
                tracker.observe(&born, tick as f64, |_| None);
            }
            let now = picture.tick as f64;
            let mut drawn = tracker.draw(now, |_| None);
            drawn.fires.extend_from_slice(&shot.fires);
            let device = &gpu.device;
            let queue = &gpu.queue;
            let devices = tore_sim::combat::countermeasures::Devices::default();
            let empty = tore_sim::combat::smoke::Smoke::default();
            gpu.sim.vapor(device, queue, &[]);
            gpu.sim
                .smoke(device, queue, &art.smoke, [&empty, &empty], &devices);
            gpu.sim
                .effects(device, queue, &art.effects, &picture.effects, &shot.marks);
            gpu.sim.emitters(queue, &devices, &[], &drawn);
            gpu.sim.combat(
                device,
                queue,
                &crate::render_snapshot::combat_geometry(
                    &picture, &art, &ownship, &start, &camera, &world, &scenery,
                ),
            );
            gpu.sim.aircraft(device, queue, &ownship, &[]);
            if night_range {
                // The night scene's haze ramp: start, end (feet), amounts.
                let fog = scenery.sample_view(&world, camera.position[1], 0).fog;
                println!("Impact preview: {} haze {fog:?}", shot.name);
            }
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
