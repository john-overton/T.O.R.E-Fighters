//! `--surface-fx-preview OUT_DIR`: offscreen renders of what the surface
//! defenses look like in the air, by day, at dusk and at night, for
//! docs/spec/surface-defenses.md#flak-bursts-gunfire-launches-and-light. A development aid: no
//! window opens, and the scenes are the real game. Each one builds the Quick
//! Mission a ground target describes, flies the player on a scripted line past
//! a chosen unit, steps the whole world (the live tick, so the surface
//! controllers shoot for real) and, a moment after the first flak burst, gun
//! round or missile, draws the frames through the same picture, tracker and
//! renderer the game uses. It also draws the flight map with the contacts the
//! player's sensors have found.
//!
//! ```text
//! tore-app --surface-fx-preview OUT_DIR [SCENE...]
//! ```
//!
//! Scenes: `flak` (the KS-19 and KS-12 flak of North Vietnam), `ks12gun` and
//! `ks19gun` (each gun's muzzle flash from the side, nr-flak), `zsu23` and
//! `zsu57` (the Ukraine city's guns), `troops` (an airfield's small arms,
//! B17), `squadkill` (the same squad killed as it opens fire, BD2), `sam` (its
//! SA-6 launching), `wreck` (a destroyed ZSU-23 smoking), `map` (the flight
//! map over it), and `ground`, `groundtank` and `groundwreck` (units, the
//! airfield squad, a parked aircraft and a wreck seen from the ground beside
//! them, by day: do they stand on it? Slice GROUND). With none named, all of
//! them.
use crate::{
    AppResult,
    aircraft::Airframe,
    camera::Camera,
    gun_flash::Tracker,
    menu::Sprite,
    reel::{Gpu, HEIGHT, WIDTH},
    render_snapshot::CombatArt,
    scenery::Scenery,
    snapshot::RenderSnapshot,
    terrain::Overrides,
};
use std::{collections::BTreeMap, path::Path};
use tore_formats::aircraft::AircraftId;
use tore_sim::{
    attitude::Vector,
    combat::{
        countermeasures::Devices,
        live::{self, EffectKind},
    },
};
use tore_world::{
    mission::{Condition, Defense, MissionSpec},
    seats::{SeatId, SeatInput},
    surface::UnitId,
    world::{Seating, TickOutput, World},
};

const FEET_PER_NM: f64 = 6_076.;
const FPS_PER_KNOT: f64 = 1.687_8;
const PLAYER: u32 = 0;
const SEAT: SeatId = SeatId(0);

/// What a scene waits for before it draws.
#[derive(Clone, Copy, PartialEq)]
enum Trigger {
    /// The first flak burst.
    Flak,
    /// The first round of a surface gun (by record name).
    Gun(&'static str),
    /// The first surface missile off its rail.
    Missile,
    /// A fixed time, seconds into the flight.
    Time(f64),
}

/// How a frame's camera is placed.
#[derive(Clone, Copy)]
enum View {
    /// Behind and above the player, looking at the newest flak burst.
    Flak,
    /// Close behind the player, looking at the aircraft, to see what a burst's
    /// light does to it.
    Jet,
    /// Beside the firing gun, looking along its barrel.
    Gun,
    /// Standing off to the side of the whole gun, looking at the gun and the
    /// flash together: does the flash come from the end of the barrel?
    Side,
    /// Low beside an airfield's squads, looking out along their fire toward
    /// the aircraft.
    Field,
    /// Close in front of an airfield squad, looking at its soldiers (alive,
    /// or the ground where they stood) with their fire going away behind.
    Squad,
    /// The squad from 1,500 ft out, 60 ft up: the range of a low pass.
    Far,
    TracerRange,
    /// Beside the launcher, looking up at it.
    Launch,
    /// Farther off, looking up the missile's path.
    Trail,
    /// Beside a destroyed unit, looking up its smoke column.
    Wreck,
    /// The flight map.
    Map,
    /// Standing on the ground this many feet from the scene's unit, looking
    /// at where it stands (alive or its wreck): is it on the ground?
    Close(f64),
    /// The same beside the first unit of this type (any side).
    Unit(&'static str, f64),
    /// The same beside the first parked aircraft.
    Parked(f64),
}
impl View {
    fn name(self) -> String {
        match self {
            View::Close(range) => format!("close{range:.0}"),
            View::Unit(stem, range) => format!("{}{range:.0}", stem.to_ascii_lowercase()),
            View::Parked(range) => format!("parked{range:.0}"),
            other => other.fixed_name().into(),
        }
    }

    fn fixed_name(self) -> &'static str {
        match self {
            View::Flak => "wide",
            View::Jet => "jet",
            View::Gun => "gun",
            View::Side => "side",
            View::Field => "field",
            View::Squad => "squad",
            View::Far => "far",
            View::TracerRange => "tracers-4nm",
            View::Launch => "pad",
            View::Trail => "trail",
            View::Wreck => "wreck",
            View::Map => "map",
            View::Close(_) | View::Unit(..) | View::Parked(_) => "stand",
        }
    }
}

struct Scene {
    name: &'static str,
    theater: &'static str,
    stem: &'static str,
    defenses: (usize, usize),
    /// The unit type the player flies past (`ZSU23`) and which of them.
    over: &'static str,
    index: usize,
    altitude: f64,
    speed_kt: f64,
    /// Feet to the east of the unit the line passes.
    pass: f64,
    from_nm: f64,
    seconds: f64,
    trigger: Trigger,
    /// Ticks after the trigger each frame is drawn.
    frames: &'static [u64],
    views: &'static [View],
    /// Vertical field of view, degrees.
    fov: f64,
    /// The muzzle class the gun view follows (`surface_fx::LOOKS` index).
    muzzle_class: usize,
    /// Destroy the unit when the scene triggers.
    kill: bool,
}

const SCENES: [Scene; 13] = [
    Scene {
        name: "flak",
        theater: "TVIET",
        stem: "QTAAA",
        defenses: (3, 0),
        over: "KS19",
        index: 0,
        altitude: 12_000.,
        speed_kt: 420.,
        pass: 0.,
        from_nm: 6.,
        seconds: 120.,
        trigger: Trigger::Flak,
        frames: &[1, 6, 30, 160, 420],
        views: &[View::Flak, View::Jet],
        fov: 55.,
        muzzle_class: 0,
        kill: false,
    },
    Scene {
        name: "ks12gun",
        theater: "TVIET",
        stem: "QTAAA",
        defenses: (3, 0),
        over: "KS12",
        index: 0,
        altitude: 9_000.,
        speed_kt: 420.,
        pass: 0.,
        from_nm: 6.,
        seconds: 120.,
        trigger: Trigger::Gun("KS12.JT"),
        frames: &[2, 6],
        views: &[View::Side, View::Gun],
        fov: 40.,
        muzzle_class: 2,
        kill: false,
    },
    Scene {
        name: "ks19gun",
        theater: "TVIET",
        stem: "QTAAA",
        defenses: (3, 0),
        over: "KS19",
        index: 0,
        altitude: 9_000.,
        speed_kt: 420.,
        pass: 0.,
        from_nm: 6.,
        seconds: 120.,
        trigger: Trigger::Gun("KS19.JT"),
        frames: &[2, 6],
        views: &[View::Side, View::Gun],
        fov: 40.,
        muzzle_class: 2,
        kill: false,
    },
    Scene {
        name: "zsu23",
        theater: "UKR",
        stem: "QUCITY",
        defenses: (3, 3),
        over: "ZSU23",
        index: 0,
        altitude: 3_000.,
        speed_kt: 420.,
        pass: 1_500.,
        from_nm: 3.,
        seconds: 120.,
        trigger: Trigger::Gun("ZSU23.JT"),
        frames: &[4, 9, 60],
        views: &[View::Gun, View::TracerRange],
        fov: 45.,
        muzzle_class: 0,
        kill: false,
    },
    Scene {
        name: "zsu57",
        theater: "UKR",
        stem: "QUCITY",
        defenses: (3, 3),
        over: "ZSU57",
        index: 0,
        altitude: 3_000.,
        speed_kt: 420.,
        pass: 1_500.,
        from_nm: 3.,
        seconds: 120.,
        trigger: Trigger::Gun("ZSU57.JT"),
        frames: &[3, 30],
        views: &[View::Gun, View::TracerRange],
        fov: 45.,
        muzzle_class: 1,
        kill: false,
    },
    Scene {
        name: "troops",
        theater: "APA",
        stem: "QAPFAIR",
        defenses: (3, 3),
        over: "TROOPS",
        index: 0,
        altitude: 400.,
        speed_kt: 350.,
        pass: 600.,
        from_nm: 2.,
        seconds: 120.,
        trigger: Trigger::Gun("SMLARMS.JT"),
        frames: &[2, 20, 50],
        views: &[View::Gun, View::Field, View::Squad, View::Far],
        fov: 18.,
        muzzle_class: 3,
        kill: false,
    },
    Scene {
        name: "squadkill",
        theater: "APA",
        stem: "QAPFAIR",
        defenses: (3, 3),
        over: "TROOPS",
        index: 0,
        altitude: 400.,
        speed_kt: 350.,
        pass: 600.,
        from_nm: 2.,
        seconds: 200.,
        trigger: Trigger::Gun("SMLARMS.JT"),
        frames: &[2, 60, 600],
        views: &[View::Squad],
        fov: 18.,
        muzzle_class: 3,
        kill: true,
    },
    Scene {
        name: "sam",
        theater: "UKR",
        stem: "QUCITY",
        defenses: (3, 3),
        over: "SA6",
        index: 0,
        altitude: 9_000.,
        speed_kt: 420.,
        pass: 3_000.,
        from_nm: 14.,
        seconds: 240.,
        trigger: Trigger::Missile,
        frames: &[2, 14, 60, 300],
        views: &[View::Launch, View::Trail],
        fov: 55.,
        muzzle_class: 0,
        kill: false,
    },
    Scene {
        name: "wreck",
        theater: "UKR",
        stem: "QUCITY",
        defenses: (3, 3),
        over: "ZSU23",
        index: 0,
        altitude: 3_000.,
        speed_kt: 420.,
        pass: 1_500.,
        from_nm: 3.,
        seconds: 400.,
        trigger: Trigger::Time(20.),
        frames: &[2, 300, 1_800, 6_000],
        views: &[View::Wreck],
        fov: 50.,
        muzzle_class: 0,
        kill: true,
    },
    Scene {
        name: "map",
        theater: "UKR",
        stem: "QUCITY",
        defenses: (3, 3),
        over: "SA6",
        index: 0,
        altitude: 9_000.,
        speed_kt: 420.,
        pass: 3_000.,
        from_nm: 14.,
        seconds: 240.,
        trigger: Trigger::Time(60.),
        frames: &[0],
        views: &[View::Map],
        fov: 55.,
        muzzle_class: 0,
        kill: false,
    },
    // Grounding (slice GROUND, 2026-10-13): the airfield squad close and
    // far, and the airfield's other units and a parked aircraft, each seen
    // from the ground beside it.
    Scene {
        name: "ground",
        theater: "APA",
        stem: "QAPFAIR",
        defenses: (3, 3),
        over: "TROOPS",
        index: 0,
        altitude: 400.,
        speed_kt: 350.,
        pass: 600.,
        from_nm: 2.,
        seconds: 30.,
        trigger: Trigger::Time(3.),
        frames: &[0],
        views: &[
            View::Close(30.),
            View::Far,
            View::Unit("TRUCK", 90.),
            View::Unit("SA6", 110.),
            View::Unit("ZSU23", 80.),
            View::Unit("M163", 80.),
            View::Parked(110.),
        ],
        fov: 30.,
        muzzle_class: 3,
        kill: false,
    },
    Scene {
        name: "groundtank",
        theater: "UKR",
        stem: "QUCITY",
        defenses: (3, 3),
        over: "T72",
        index: 0,
        altitude: 3_000.,
        speed_kt: 420.,
        pass: 1_500.,
        from_nm: 3.,
        seconds: 30.,
        trigger: Trigger::Time(3.),
        frames: &[0],
        views: &[
            View::Close(90.),
            View::Unit("T80", 90.),
            View::Unit("2S6", 90.),
        ],
        fov: 30.,
        muzzle_class: 0,
        kill: false,
    },
    Scene {
        name: "groundwreck",
        theater: "UKR",
        stem: "QUCITY",
        defenses: (3, 3),
        over: "ZSU23",
        index: 0,
        altitude: 3_000.,
        speed_kt: 420.,
        pass: 1_500.,
        from_nm: 3.,
        seconds: 60.,
        trigger: Trigger::Time(3.),
        frames: &[1_200],
        views: &[View::Close(70.)],
        fov: 30.,
        muzzle_class: 0,
        kill: true,
    },
];

/// The camera standing `range` feet from a ground spot `at`, a little above
/// it, looking at it: whether what stands there meets the ground.
fn standing_view(at: Vector, range: f64, fov: f64) -> Camera {
    let eye = [
        at[0] - range * 0.6,
        at[1] + (range * 0.08).max(3.),
        at[2] + range * 0.8,
    ];
    look_at(eye, [at[0], at[1] + range * 0.06, at[2]], fov)
}

/// The ground under the first unit of type `stem` (any side).
fn unit_spot(world: &World, stem: &str) -> Option<Vector> {
    let unit = world.terrain.surface.units.iter().find(|u| {
        u.resource
            .split('.')
            .next()
            .is_some_and(|s| s.eq_ignore_ascii_case(stem))
    })?;
    let [x, y, z] = unit.position.map(f64::from);
    Some([
        x,
        f64::from(world.terrain.height(x as f32, z as f32)) + y,
        z,
    ])
}

/// The light a sheet is drawn in: a name, the weather condition and the clock.
const LIGHTS: [(&str, Condition, Option<[i32; 2]>); 3] = [
    ("day", Condition::Clear, Some([13, 0])),
    ("dusk", Condition::Sunset, None),
    ("night", Condition::Clear, Some([23, 0])),
];

pub fn run() -> AppResult<()> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let Some((out, names)) = args.split_first() else {
        return Err(
            "--surface-fx-preview OUTPUT_DIRECTORY [flak|zsu23|zsu57|troops|squadkill|sam|wreck|map|ground|groundtank|groundwreck ...]"
                .into(),
        );
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let resources = crate::surface_trace::resources()?;
    let assets = crate::reel::load_assets()?;
    for scene in SCENES
        .iter()
        .filter(|s| names.is_empty() || names.iter().any(|n| n == s.name))
    {
        for (light, condition, time) in LIGHTS {
            // The map has no light of its own: one sheet; the grounding
            // scenes are drawn by day.
            if (matches!(scene.views, [View::Map]) || scene.name.starts_with("ground"))
                && light != "day"
            {
                continue;
            }
            render(scene, light, condition, time, &resources, &assets, out)?;
        }
    }
    Ok(())
}

fn spec(scene: &Scene, condition: Condition, time: Option<[i32; 2]>) -> AppResult<MissionSpec> {
    let mut spec = MissionSpec::new(scene.theater, AircraftId::F18);
    spec.condition = condition;
    spec.weather = Overrides {
        time,
        wind: None,
        cloud_altitude: Some(0),
        redrawn_airports: crate::scenery::redrawn_airports()?,
        city_fill: crate::scenery::city_fill()?,
        city_fill_cap: crate::scenery::city_fill_cap()?,
    };
    spec.ground_target = Some(scene.stem.into());
    spec.aaa = Defense::from_level(scene.defenses.0).ok_or("defense level")?;
    spec.sam = Defense::from_level(scene.defenses.1).ok_or("defense level")?;
    spec.surface_seed = 1;
    spec.enemy_nationality = tore_formats::quick_template::tables::ENEMY_NATIONALITY
        [tore_formats::quick_template::tables::THEATERS
            .iter()
            .position(|t| t.eq_ignore_ascii_case(scene.theater))
            .ok_or("theater")?] as u8;
    spec.cheats.damage = tore_sim::cheats::Damage::Invulnerable;
    spec.validate()?;
    Ok(spec)
}

/// The unit the player flies past: the `index`th hostile one of its type.
fn subject(world: &World, scene: &Scene) -> AppResult<UnitId> {
    let player_side = world
        .combat
        .state
        .ownship(PLAYER)
        .map_or(live::DEFAULT_OWNSHIP_SIDE, |own| own.side);
    world
        .terrain
        .surface
        .units
        .iter()
        .filter(|u| {
            u.resource
                .split('.')
                .next()
                .is_some_and(|stem| stem.eq_ignore_ascii_case(scene.over))
                && u.side != player_side
        })
        .nth(scene.index)
        .map(|u| u.id)
        .ok_or_else(|| format!("{} has no {} to fly past", scene.stem, scene.over).into())
}

fn look_at(eye: Vector, target: Vector, fov: f64) -> Camera {
    let mut camera = Camera::new();
    camera.position = eye;
    let d: Vector = std::array::from_fn(|i| target[i] - eye[i]);
    camera.yaw = d[0].atan2(d[2]) as f32;
    camera.pitch = d[1].atan2(d[0].hypot(d[2])) as f32;
    camera.zoom = (30_f64.to_radians().tan() / (fov / 2.).to_radians().tan()) as f32;
    camera
}

/// A missile whose motor has lit (the launch the tracker draws).
fn lit(p: &crate::snapshot::ProjectilePose) -> bool {
    f64::from(p.speed_f8) / 256. >= crate::surface_fx::launch::MOTOR_FPS
}

/// Whether the picture holds what the scene waits for.
fn triggered(trigger: Trigger, picture: &RenderSnapshot, time: f64) -> bool {
    match trigger {
        Trigger::Flak => picture.effects.iter().any(|e| e.kind == EffectKind::Flak),
        Trigger::Gun(record) => picture
            .projectiles
            .iter()
            .any(|p| p.gun && crate::surface_fx::is_unit(p.owner) && p.weapon == record),
        Trigger::Missile => picture
            .projectiles
            .iter()
            .any(|p| !p.gun && crate::surface_fx::is_unit(p.owner) && lit(p)),
        Trigger::Time(seconds) => time >= seconds,
    }
}

fn sprites(assets: &crate::assets::Assets) -> BTreeMap<String, Sprite> {
    let mut sprites = BTreeMap::new();
    let Some(palette) = assets
        .pics
        .get("QUIKMIS3.PIC")
        .and_then(|p| <[[u8; 3]; 256]>::try_from(p.palette.clone()).ok())
    else {
        return sprites;
    };
    for name in [
        "MCICONS.PIC",
        "ACTION0L.PIC",
        "ACTION0M.PIC",
        "ACTION0R.PIC",
    ] {
        if let Some(p) = assets.pics.get(name) {
            sprites.insert(
                name.to_owned(),
                Sprite {
                    width: p.width,
                    height: p.height,
                    rgba: p.rgba(&palette),
                    glyphs: p.glyphs.clone(),
                },
            );
        }
    }
    sprites
}

fn render(
    scene: &Scene,
    light: &str,
    condition: Condition,
    time: Option<[i32; 2]>,
    resources: &BTreeMap<String, Vec<u8>>,
    assets: &crate::assets::Assets,
    out: &Path,
) -> AppResult<()> {
    let spec = spec(scene, condition, time)?;
    let mut world =
        crate::surface_dump::world_without_front(&spec, resources, Seating::SinglePlayer)?;
    if let Some(why) = &world.terrain.surface.unresolved {
        return Err(why.clone().into());
    }
    let unit = subject(&world, scene)?;
    let center = world
        .terrain
        .surface
        .arsenal
        .arms(unit)
        .ok_or("the unit has no arms")?
        .position;
    let forward = [0., 0., 1.];
    let speed = scene.speed_kt * FPS_PER_KNOT;
    let start: Vector = [
        center[0] + scene.pass,
        center[1],
        center[2] - scene.from_nm * FEET_PER_NM,
    ];
    let altitude = center[1] + scene.altitude;
    let mut scenery = Scenery::build(resources, &world.terrain)?;
    let mut gpu = pollster::block_on(Gpu::new(&scenery))?;
    let mut art = CombatArt::load(resources)?;
    let ownship = Airframe::load(resources, AircraftId::F18)?;
    let sprites = sprites(assets);
    let mut tracker = Tracker::default();
    let mut output = TickOutput::default();
    let ticks = (scene.seconds * 120.) as u64;
    let last = scene.frames.iter().copied().max().unwrap_or(0);
    let mut p_seen: std::collections::BTreeSet<String> = Default::default();
    let mut fired_at: Option<u64> = None;
    // Where the launcher stands, for the pad views.
    let mut pad = center;
    let mut taken = 0;
    for tick in 0..ticks {
        let t = tick as f64 / 120.;
        {
            let flight = &mut world.cockpits[0].flight;
            flight.position = [
                start[0] + forward[0] * speed * t,
                altitude,
                start[2] + forward[2] * speed * t,
            ];
            flight.velocity = [forward[0] * speed, 0., forward[2] * speed];
            flight.speed = speed;
            flight.yaw = 0.;
            flight.pitch = 0.;
            flight.bank = 0.;
            flight.vertical_speed = 0.;
            flight.roll_rate = 0.;
            flight.pitch_rate = 0.;
        }
        let input = SeatInput {
            seat: SEAT,
            tick: world.tick(),
            ..Default::default()
        };
        world.step(std::slice::from_ref(&input), &mut output)?;
        let picture = world.combat.render_snapshot().clone();
        tracker.observe(&picture, tick as f64, |_| None);
        if fired_at.is_none() && triggered(scene.trigger, &picture, t) {
            fired_at = Some(tick);
            if scene.kill
                && let Some(target) = world
                    .combat
                    .state
                    .targets
                    .iter_mut()
                    .find(|target| target.id == unit.0)
            {
                target.hp = 0;
            }
            // The launcher that fired: its own spot is the pad.
            if let Some(p) = picture
                .projectiles
                .iter()
                .find(|p| !p.gun && crate::surface_fx::is_unit(p.owner) && lit(p))
                && let Some(arms) = world.terrain.surface.arsenal.arms(UnitId(p.owner))
            {
                pad = arms.position;
            }
        }
        let Some(first) = fired_at else { continue };
        let Some(&offset) = scene.frames.get(taken) else {
            break;
        };
        if tick < first + offset {
            continue;
        }
        taken += 1;
        let state = world.cockpits[0].flight.clone();
        for p in picture.projectiles.iter().filter(|p| !p.gun) {
            println!(
                "  missile {:#x} owner {:#x} {} at {:?} speed {:.0} ft/s",
                p.id,
                p.owner,
                p.weapon,
                p.position.map(|v| v.round()),
                f64::from(p.speed_f8) / 256.
            );
        }
        let now = tick as f64;
        if matches!(scene.views, [View::Map]) {
            let name = format!("{}-{light}-t{offset}", scene.name);
            let readout = world
                .combat
                .cockpit_readout(
                    PLAYER,
                    tore_world::combat::launcher(&state),
                    world.ai_wings.as_ref(),
                    world.cockpits.first(),
                )
                .ok_or("no readout")?;
            let mut map = crate::flight_map::Map::default();
            for key in ["+", "+"] {
                map.key(key);
            }
            let mut pixels = vec![0_u8; 640 * 480 * 4];
            map.draw(
                &mut pixels,
                &world.terrain,
                &scenery,
                &state,
                &readout,
                live::DEFAULT_OWNSHIP_SIDE,
                &ownship.font,
                &sprites,
                None,
            );
            let path = out.join(format!("{name}.png"));
            std::fs::write(&path, crate::replay::png::encode_rgba(640, 480, &pixels)?)?;
            println!(
                "Surface fx preview: {} ({} map contacts)",
                path.display(),
                readout.map.len()
            );
            if taken == scene.frames.len() {
                break;
            }
            continue;
        }
        let tracker_now = tracker.draw(now, |_| None);
        for &view in scene.views {
            let name = format!("{}-{light}-{}-t{offset}", scene.name, view.name());
            let camera = match view {
                View::Flak => {
                    let burst = picture
                        .effects
                        .iter()
                        .filter(|e| e.kind == EffectKind::Flak)
                        .map(|e| e.position)
                        .min_by(|a, b| {
                            let d = |p: &Vector| {
                                (0..3)
                                    .map(|i| (p[i] - state.position[i]).powi(2))
                                    .sum::<f64>()
                            };
                            d(a).total_cmp(&d(b))
                        })
                        .unwrap_or([
                            state.position[0],
                            state.position[1],
                            state.position[2] + 800.,
                        ]);
                    let eye = [
                        state.position[0] + 140.,
                        state.position[1] + 70.,
                        state.position[2] - 360.,
                    ];
                    look_at(eye, burst, scene.fov)
                }
                View::Side => {
                    let (muzzle, direction) = tracker_now
                        .flashes
                        .iter()
                        .filter(|f| f.kind as usize == scene.muzzle_class)
                        .max_by(|a, b| a.intensity.total_cmp(&b.intensity))
                        .map(|f| (f.position, f.direction))
                        .unwrap_or((center, [0., 1., 0.]));
                    let side = [-direction[2], 0., direction[0]];
                    let norm = side[0].hypot(side[2]).max(1e-6);
                    let middle = [
                        (muzzle[0] + center[0]) / 2.,
                        (muzzle[1] + center[1]) / 2. + 6.,
                        (muzzle[2] + center[2]) / 2.,
                    ];
                    let eye = [
                        middle[0] + side[0] / norm * 85.,
                        middle[1],
                        middle[2] + side[2] / norm * 85.,
                    ];
                    look_at(eye, middle, scene.fov)
                }
                View::Gun => {
                    // The brightest flash showing: a gun that has just fired.
                    let (muzzle, direction) = tracker_now
                        .flashes
                        .iter()
                        .filter(|f| f.kind as usize == scene.muzzle_class)
                        .max_by(|a, b| a.intensity.total_cmp(&b.intensity))
                        .map(|f| (f.position, f.direction))
                        .unwrap_or((center, [0., 1., 0.]));
                    let side = [-direction[2], 0., direction[0]];
                    let norm = side[0].hypot(side[2]).max(1e-6);
                    let eye = [
                        muzzle[0] + side[0] / norm * 120.,
                        muzzle[1] + 8.,
                        muzzle[2] + side[2] / norm * 120.,
                    ];
                    let target = [
                        muzzle[0] + direction[0] * 40.,
                        muzzle[1] + direction[1] * 40.,
                        muzzle[2] + direction[2] * 40.,
                    ];
                    look_at(eye, target, scene.fov)
                }
                View::Field => {
                    // Behind and beside the newest squad round's gun, looking
                    // down its fire: the tracers climb away from the camera.
                    let round = picture.projectiles.iter().rev().find(|p| {
                        p.gun && crate::surface_fx::is_unit(p.owner) && p.weapon == "SMLARMS.JT"
                    });
                    let (base, dir) = round
                        .and_then(|p| {
                            let arms = world.terrain.surface.arsenal.arms(UnitId(p.owner))?;
                            Some((arms.position, p.direction))
                        })
                        .unwrap_or((center, [0., 0., 1.]));
                    let side = [-dir[2], 0., dir[0]];
                    let norm = side[0].hypot(side[2]).max(1e-6);
                    let eye = [
                        base[0] - dir[0] * 40. + side[0] / norm * 30.,
                        base[1] + 9.,
                        base[2] - dir[2] * 40. + side[2] / norm * 30.,
                    ];
                    let aim = [
                        base[0] + dir[0] * 250.,
                        base[1] + dir[1] * 250.,
                        base[2] + dir[2] * 250.,
                    ];
                    look_at(eye, aim, 32.)
                }
                View::Squad => {
                    // From 55 ft out on the squad's far side from the jet's
                    // line (west and north), 5 ft up, looking at the men.
                    let eye = [center[0] - 30., center[1] + 5., center[2] + 50.];
                    look_at(eye, [center[0], center[1] + 3., center[2]], 38.)
                }
                View::TracerRange => {
                    let aim = picture
                        .projectiles
                        .iter()
                        .find(|p| p.gun && p.tracer)
                        .map_or(center, |p| p.position);
                    look_at([aim[0] - 4. * 6076.12, aim[1], aim[2]], aim, 60.)
                }
                View::Far => {
                    let eye = [center[0] - 1_500., center[1] + 60., center[2]];
                    look_at(eye, [center[0], center[1] + 20., center[2]], 24.)
                }
                View::Launch | View::Trail => {
                    if matches!(view, View::Trail) {
                        let eye = [pad[0] + 1_300., pad[1] + 60., pad[2] - 900.];
                        // The missile in flight if there is one, else the sky over the pad.
                        let aim = picture
                            .projectiles
                            .iter()
                            .rev()
                            .find(|p| !p.gun && crate::surface_fx::is_unit(p.owner) && lit(p))
                            .map_or([pad[0], pad[1] + 520., pad[2]], |p| p.position);
                        look_at(eye, aim, scene.fov)
                    } else {
                        let eye = [pad[0] + 230., pad[1] + 22., pad[2] - 80.];
                        look_at(eye, [pad[0], pad[1] + 45., pad[2]], scene.fov)
                    }
                }
                View::Wreck => {
                    let eye = [center[0] + 330., center[1] + 40., center[2] - 240.];
                    look_at(eye, [center[0], center[1] + 160., center[2]], scene.fov)
                }
                View::Jet => {
                    let eye = [
                        state.position[0] + 26.,
                        state.position[1] + 10.,
                        state.position[2] - 80.,
                    ];
                    look_at(eye, state.position, 42.)
                }
                View::Close(range) => {
                    // The unit's own ground, not its weapons' height.
                    let [x, _, z] = center;
                    let at = [x, f64::from(world.terrain.height(x as f32, z as f32)), z];
                    standing_view(at, range, scene.fov)
                }
                View::Unit(stem, range) => {
                    standing_view(unit_spot(&world, stem).unwrap_or(center), range, scene.fov)
                }
                View::Parked(range) => {
                    let height =
                        |x: f64, z: f64| f64::from(world.terrain.height(x as f32, z as f32));
                    let (poses, _) = tore_world::surface::parked::place(
                        resources,
                        &world.terrain.surface,
                        &height,
                    );
                    let at = poses.first().map_or(center, |pose| pose.ground);
                    standing_view(at, range, scene.fov)
                }
                View::Map => unreachable!(),
            };
            // Each weapon shape once; a gun's tracer marker has none to draw.
            let new: Vec<&str> = picture
                .projectiles
                .iter()
                .filter_map(|p| p.shape.as_deref())
                .filter(|name| !p_seen.contains(*name))
                .collect();
            art.add_shapes(new.iter().copied(), resources);
            p_seen.extend(new.into_iter().map(str::to_owned));
            scenery.resolve_palette(&world.terrain, camera.position[1]);
            scenery.set_origin(camera.position);
            let geometry = std::sync::Arc::clone(
                scenery.static_geometry(&world.combat.state.targets, &world.combat.surface),
            );
            gpu.sim.airports(&gpu.device, &gpu.queue, &geometry);
            // The men and the units that turn, drawn every frame as the game
            // draws them.
            let standing = crate::render_snapshot::standing(&world.combat.state.targets);
            gpu.sim.surface_units(
                &gpu.device,
                &gpu.queue,
                &scenery.surface_vertices(&picture, &|id| standing.contains(&id), &camera),
            );
            let device = &gpu.device;
            let queue = &gpu.queue;
            let devices = Devices::default();
            gpu.sim.vapor(device, queue, &[]);
            gpu.sim.smoke(
                device,
                queue,
                &art.smoke,
                [&world.combat.state.smoke, &world.combat.contrails],
                &devices,
            );
            gpu.sim.effects(
                device,
                queue,
                &art.effects,
                &picture.effects,
                &picture.marks,
            );
            let guns = tracker_now.clone();
            if matches!(view, View::Jet) {
                for light in &guns.lights {
                    let d = (0..3)
                        .map(|i| (light.position[i] - state.position[i]).powi(2))
                        .sum::<f64>()
                        .sqrt();
                    println!(
                        "  light {:.0} at {:.0} ft from the aircraft: {:.2} of full sun",
                        light.strength,
                        d,
                        light.strength / (d * d + 25.)
                    );
                }
            }
            for flash in &guns.flashes {
                println!(
                    "  flash kind {} intensity {:.2} length {:.1} at {:?} -> pixel {:?}",
                    flash.kind,
                    flash.intensity,
                    flash.length,
                    flash.position.map(|v| v.round()),
                    camera.project([WIDTH, HEIGHT], flash.position)
                );
            }
            gpu.sim.emitters(queue, &devices, &[], &guns);
            gpu.sim.combat(
                device,
                queue,
                &crate::render_snapshot::combat_geometry(
                    &picture,
                    &art,
                    &ownship,
                    &state,
                    &camera,
                    &world.terrain,
                    &scenery,
                ),
            );
            let vertices = ownship.vertices(&state, &camera, &world.terrain, &scenery);
            gpu.sim.aircraft(device, queue, &ownship, &vertices);
            let pixels = gpu.pixels(&camera, &world.terrain, &scenery)?;
            let path = out.join(format!("{name}.png"));
            std::fs::write(
                &path,
                crate::replay::png::encode_rgba(WIDTH, HEIGHT, &pixels)?,
            )?;
            println!(
                "Surface fx preview: {} (tick {tick}: {} flashes, {} lights, {} puffs, {} effects)",
                path.display(),
                guns.flashes.len(),
                guns.lights.len(),
                guns.puffs.len(),
                picture.effects.len(),
            );
        }
        if taken == scene.frames.len() || tick >= first + last {
            break;
        }
    }
    if fired_at.is_none() {
        println!(
            "Surface fx preview: {} {light}: the trigger never came in {} s",
            scene.name, scene.seconds
        );
    }
    Ok(())
}
