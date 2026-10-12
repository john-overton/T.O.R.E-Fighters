//! `--a2g-radar THEATER STEM [--aircraft ID] [--page ag|ir|harm] [--from NM]
//! [--altitude FT] [--seconds S] [--condition N] [--surface-seed N]
//! [--defenses AAA SAM] [--designate next|previous|visual|nearest]
//! [--easy-targeting] [--airfield ID] [--render OUT.png]`: a development aid
//! and the battery's check of the ground pages (docs/spec/air-to-ground.md,
//! "The air-to-ground radar", "FLIR ground contacts", "The HARM scope page").
//! No window opens; the whole mission tick runs as in the game.
//!
//! The player flies a scripted straight, level line at the ground target from
//! `--from` nautical miles out, invulnerable. On the first tick the page is
//! selected through the player's own controls: `ag` sends the A-G command
//! (Ctrl+R), `ir` and `harm` set the scope channel (M). Two seconds in the
//! list is printed, then `--designate` gives one designation command through
//! the seat (T, Shift+T, Enter or Ctrl+X) and the selection and the name the
//! target window shows are printed; at the end the list and the selection
//! are printed again. `--airfield ID` tells combat that aircraft `ID` is in
//! an airfield sequence, as the AI does on takeoff and landing; `nearest-ai`
//! names the AI aircraft nearest the player (the wingman), and
//! `--look-at-ai` points the player's nose (not its path) at it.
//! `--render` writes the instrument page as the cockpit draws it, four times
//! the size. Every line starts `a2g-radar:`.
use crate::{AppResult, aircraft, instruments, scope, surface_dump};
use tore_formats::aircraft::AircraftId;
use tore_sim::{
    attitude::Vector,
    combat::live::{self, Command},
    sensors::{Channel, Controls, GroundSymbol},
};
use tore_world::{
    combat::launcher,
    mission::{Condition, Defense, MissionSpec},
    seats::{SeatCommand, SeatId, SeatInput},
    world::{Seating, TickOutput, World},
};

const SEAT: SeatId = SeatId(0);
const FEET_PER_NM: f64 = 6_076.12;
const FPS_PER_KNOT: f64 = 1.687_8;

struct Options {
    theater: String,
    stem: String,
    aircraft: AircraftId,
    page: Channel,
    air_ground: bool,
    from_nm: f64,
    altitude: f64,
    seconds: f64,
    condition: Condition,
    seed: u32,
    defenses: (usize, usize),
    designate: Option<Command>,
    easy_targeting: bool,
    airfield: Vec<u32>,
    airfield_nearest: bool,
    look_at_ai: bool,
    range_index: usize,
    render: Option<std::path::PathBuf>,
}

fn options() -> AppResult<Options> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let mut o = Options {
        theater: String::new(),
        stem: String::new(),
        aircraft: AircraftId::F18,
        page: Channel::Radar,
        air_ground: true,
        from_nm: 8.,
        altitude: 5_000.,
        seconds: 4.,
        condition: Condition::Clear,
        seed: 1,
        defenses: (0, 0),
        designate: Some(Command::Designate),
        easy_targeting: false,
        airfield: Vec::new(),
        airfield_nearest: false,
        look_at_ai: false,
        range_index: Controls::default().range_index,
        render: None,
    };
    let mut positional = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut next = || {
            it.next()
                .ok_or_else(|| format!("{arg} needs a value"))
                .cloned()
        };
        match arg.as_str() {
            "--aircraft" => o.aircraft = AircraftId::parse(&next()?)?,
            "--page" => {
                (o.page, o.air_ground) = match next()?.as_str() {
                    "ag" => (Channel::Radar, true),
                    "ir" => (Channel::Infrared, false),
                    "harm" => (Channel::Harm, false),
                    other => return Err(format!("--page ag|ir|harm, not {other}").into()),
                }
            }
            "--from" => o.from_nm = next()?.parse()?,
            "--altitude" => o.altitude = next()?.parse()?,
            "--seconds" => o.seconds = next()?.parse()?,
            "--condition" => {
                o.condition = Condition::from_index(next()?.parse()?)
                    .ok_or("--condition 0 clear, 1 cloudy, 2 foggy, 3 dawn, 4 sunset, 5 night")?
            }
            "--surface-seed" => o.seed = next()?.parse()?,
            "--defenses" => o.defenses = (next()?.parse()?, next()?.parse()?),
            "--designate" => {
                o.designate = match next()?.as_str() {
                    "next" => Some(Command::Designate),
                    "previous" => Some(Command::DesignatePrevious),
                    "visual" => Some(Command::DesignateVisual),
                    "nearest" => Some(Command::NearestGround),
                    "none" => None,
                    other => return Err(format!("unknown designation {other}").into()),
                }
            }
            "--easy-targeting" => o.easy_targeting = true,
            "--look-at-ai" => o.look_at_ai = true,
            "--range" => {
                let nmi: f64 = next()?.parse()?;
                o.range_index = tore_sim::sensors::RANGE_LADDER_NMI
                    .iter()
                    .position(|v| *v == nmi)
                    .ok_or("--range needs 5, 10, 25, 50, 100 or 150")?;
            }
            "--airfield" => {
                let value = next()?;
                if value == "nearest-ai" {
                    o.airfield_nearest = true;
                    continue;
                }
                let id = match value.strip_prefix("0x") {
                    Some(hex) => u32::from_str_radix(hex, 16)?,
                    None => value.parse()?,
                };
                o.airfield.push(id);
            }
            "--render" => o.render = Some(next()?.into()),
            other if other.starts_with("--") => return Err(format!("unknown {other}").into()),
            other => positional.push(other.to_owned()),
        }
    }
    let [theater, stem] = positional.as_slice() else {
        return Err("--a2g-radar THEATER STEM [--aircraft ID] [--page ag|ir|harm] [--from NM] [--altitude FT] [--seconds S] [--condition N] [--surface-seed N] [--defenses AAA SAM] [--designate next|previous|visual|nearest|none] [--easy-targeting] [--airfield ID|nearest-ai] [--look-at-ai] [--range NM] [--render OUT.png]".into());
    };
    o.theater = theater.to_ascii_uppercase();
    o.stem = stem.trim_start_matches('~').to_ascii_uppercase();
    Ok(o)
}

fn symbol(symbol: GroundSymbol) -> &'static str {
    match symbol {
        GroundSymbol::Dot => "dot",
        GroundSymbol::Cross => "x",
        GroundSymbol::Square => "square",
    }
}

/// The page's list and the selection, as the battery reads them.
fn print_page(world: &World, controls: Controls, t: f64, label: &str) {
    let flight = &world.cockpits[0].flight;
    let Some(readout) = world.cockpit_readout(SEAT, launcher(flight)) else {
        println!("a2g-radar: t={t:.1} no readout");
        return;
    };
    let sensors = &readout.sensors;
    let mode = sensors
        .mode_for(controls.channel, &controls)
        .map_or("none", |m| m.label());
    println!(
        "a2g-radar: t={t:.1} {label} mode {mode} air-ground {} contacts {} ground {} radar-emitting {}",
        u8::from(sensors.air_ground),
        sensors.contacts.len(),
        sensors.ground.len(),
        u8::from(launcher(flight).radar && !sensors.air_ground),
    );
    for g in &sensors.ground {
        let speed_kt = g.contact.velocity[0].hypot(g.contact.velocity[2]) / FPS_PER_KNOT;
        println!(
            "a2g-radar: contact {:#010x} {} class {:#06x} symbol {} distance {:.2} bearing {:.1} speed {:.0} visible {} aircraft {}",
            g.id(),
            name(world, g.id()),
            g.class,
            symbol(g.symbol()),
            g.contact.distance_ft / FEET_PER_NM,
            g.contact.bearing_rad.to_degrees(),
            speed_kt,
            u8::from(g.visible),
            u8::from(g.aircraft),
        );
    }
    match readout.targets.designated {
        Some(id) => println!(
            "a2g-radar: t={t:.1} selected {id:#010x} {} window {}",
            name(world, id),
            readout
                .targets
                .display
                .as_ref()
                .map_or("none".into(), |row| name(world, row.id)),
        ),
        None => println!("a2g-radar: t={t:.1} selected none"),
    }
}

/// The AI aircraft nearest the player.
fn nearest_ai(world: &World) -> Option<u32> {
    let here = world.cockpits[0].flight.position;
    world
        .combat
        .state
        .targets
        .iter()
        .filter(|t| t.aircraft.is_some() && t.hp > 0)
        .map(|t| {
            let d: f64 = (0..3).map(|i| (t.position[i] - here[i]).powi(2)).sum();
            (d, t.id)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Every flying aircraft: where it is from the player and how fast.
fn print_aircraft(world: &World) {
    let here = world.cockpits[0].flight.position;
    for t in world
        .combat
        .state
        .targets
        .iter()
        .filter(|t| t.aircraft.is_some())
    {
        let d: Vector = std::array::from_fn(|i| t.position[i] - here[i]);
        println!(
            "a2g-radar: aircraft-row {:#010x} {:?} distance {:.2} bearing {:.1} height {:.0} speed {:.0} airfield {}",
            t.id,
            t.aircraft.expect("filtered"),
            (d[0].powi(2) + d[1].powi(2) + d[2].powi(2)).sqrt() / FEET_PER_NM,
            d[0].atan2(d[2]).to_degrees(),
            d[1],
            t.velocity.iter().map(|v| v * v).sum::<f64>().sqrt() / FPS_PER_KNOT,
            u8::from(world.combat.state.airfield_attached(t.id)),
        );
    }
}

/// What the target window calls a ground object or an aircraft.
fn name(world: &World, id: u32) -> String {
    world
        .combat
        .ground_name(id)
        .map(|n| n.replace(' ', "_"))
        .or_else(|| {
            world
                .combat
                .state
                .targets
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.aircraft)
                .map(|a| format!("{a:?}"))
        })
        .unwrap_or_else(|| "unnamed".into())
}

fn render(
    world: &World,
    controls: Controls,
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
    id: AircraftId,
    path: &std::path::Path,
) -> AppResult<()> {
    let airframe = aircraft::Airframe::load(resources, id)?;
    let flight = &world.cockpits[0].flight;
    let readout = world
        .cockpit_readout(SEAT, launcher(flight))
        .ok_or("no readout to render")?;
    let mut panels = instruments::Instruments::default();
    panels.palette = airframe.daylight_palette();
    panels.combat = Some(instruments::CombatReadout {
        scope: scope::scope(&readout, flight, controls),
        ..Default::default()
    });
    let raster = panels.page(9, &airframe, flight);
    const SCALE: usize = 4;
    let (w, h) = (instruments::WIDTH, instruments::HEIGHT);
    let mut rgba = Vec::with_capacity(w * h * SCALE * SCALE * 4);
    for y in 0..h * SCALE {
        for x in 0..w * SCALE {
            let at = ((y / SCALE) * w + x / SCALE) * 4;
            rgba.extend_from_slice(&raster.pixels[at..at + 4]);
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        path,
        crate::replay::png::encode_rgba((w * SCALE) as u32, (h * SCALE) as u32, &rgba)?,
    )?;
    println!("a2g-radar: rendered {}", path.display());
    Ok(())
}

pub fn run() -> AppResult<()> {
    let o = options()?;
    let mut resources = crate::reel::load_assets()?.theater_resources;
    surface_dump::with_retail_surface(&mut resources)?;
    let mut spec = MissionSpec::new(&o.theater, o.aircraft);
    spec.ground_target = Some(o.stem.clone());
    spec.condition = o.condition;
    spec.aaa = Defense::from_level(o.defenses.0).ok_or("defense levels are 0 to 3")?;
    spec.sam = Defense::from_level(o.defenses.1).ok_or("defense levels are 0 to 3")?;
    spec.surface_seed = o.seed;
    spec.cheats.damage = tore_sim::cheats::Damage::Invulnerable;
    spec.cheats.easy_targeting = o.easy_targeting;
    // A wingman: a fighter in free flight near the player, which the A-G page
    // never lists unless combat is told it is in an airfield sequence.
    spec.wings[0].count = 2;
    spec.validate()?;
    let mut world =
        crate::surface_dump::world_without_front(&spec, &resources, Seating::SinglePlayer)?;
    if let Some(why) = &world.terrain.surface.unresolved {
        return Err(format!("the ground target stands nowhere: {why}").into());
    }
    let surface = world.terrain.surface.clone();
    let units: Vec<u32> = surface.units.iter().map(|u| u.id.0).collect();
    let rows: Vec<Vector> = world
        .combat
        .state
        .targets
        .iter()
        .filter(|t| units.contains(&t.id))
        .map(|t| t.position)
        .collect();
    if rows.is_empty() {
        return Err("the ground target has no units".into());
    }
    let center: Vector =
        std::array::from_fn(|i| rows.iter().map(|r| r[i]).sum::<f64>() / rows.len() as f64);
    let listed = surface
        .units
        .iter()
        .filter(|u| {
            world
                .combat
                .state
                .ground_listing(u.id.0)
                .is_some_and(|l| l.flags & 0x20 != 0 && l.max_visible >= 75)
        })
        .count();
    let own = world
        .combat
        .state
        .ownship(world.cockpits[0].plane.0)
        .ok_or("no ownship")?;
    println!(
        "a2g-radar: aircraft {:?} radar {} infrared {} visual {} units {} listed-types {} movers {}",
        o.aircraft,
        u8::from(own.sensors.available(Channel::Radar)),
        u8::from(own.sensors.available(Channel::Infrared)),
        u8::from(own.sensors.available(Channel::Visual)),
        units.len(),
        listed,
        surface.units.iter().filter(|u| u.route.is_some()).count(),
    );
    let heading = 0f64;
    let forward = [heading.sin(), 0., heading.cos()];
    let speed = 400. * FPS_PER_KNOT;
    let start: Vector = std::array::from_fn(|i| center[i] - forward[i] * o.from_nm * FEET_PER_NM);
    let ground = f64::from(world.terrain.height(center[0] as f32, center[2] as f32));
    let altitude = ground + o.altitude;
    let controls = Controls {
        channel: o.page,
        range_index: o.range_index,
        ..Controls::default()
    };
    let mut out = TickOutput::default();
    let ticks = (o.seconds * 120.) as u64;
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
            flight.yaw = heading;
            if o.look_at_ai
                && let Some(row) = nearest_ai(&world)
                    .and_then(|id| world.combat.state.targets.iter().find(|t| t.id == id))
            {
                let flight = &world.cockpits[0].flight;
                let yaw = (row.position[0] - flight.position[0])
                    .atan2(row.position[2] - flight.position[2]);
                world.cockpits[0].flight.yaw = yaw;
            }
            let flight = &mut world.cockpits[0].flight;
            flight.pitch = 0.;
            flight.bank = 0.;
            flight.vertical_speed = 0.;
            flight.roll_rate = 0.;
            flight.pitch_rate = 0.;
            flight.radar = true;
        }
        let mut attached = o.airfield.clone();
        if o.airfield_nearest
            && let Some(id) = nearest_ai(&world)
        {
            attached.push(id);
        }
        for id in attached {
            world.combat.state.set_airfield_attached(id, true);
        }
        if tick == 239 {
            print_aircraft(&world);
        }
        let mut commands = Vec::new();
        if tick == 0 {
            let weather = world.terrain.weather.sensor_weather();
            for band in &weather.bands {
                println!(
                    "a2g-radar: weather {:.0}..{:.0} ft effects {:?}",
                    band.low_ft, band.high_ft, band.effects
                );
            }
            if o.air_ground {
                commands.push(SeatCommand::Combat(live::Command::ToggleAirGround));
            }
        }
        if tick == 240
            && let Some(command) = o.designate
        {
            print_page(&world, controls, t, "page");
            commands.push(SeatCommand::Combat(command));
        }
        let input = SeatInput {
            seat: SEAT,
            tick: world.tick(),
            sensors: controls,
            commands,
            ..SeatInput::default()
        };
        world.step(std::slice::from_ref(&input), &mut out)?;
        if tick == 240 {
            print_page(&world, controls, t, "designated");
        }
    }
    print_page(&world, controls, o.seconds, "end");
    if let Some(path) = &o.render {
        render(&world, controls, &resources, o.aircraft, path)?;
    }
    println!("a2g-radar: done at {:.0} s", o.seconds);
    Ok(())
}
