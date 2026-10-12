//! `--surface-battle`: a development aid and the battery's check of surface
//! units fighting each other (docs/spec/surface-defenses.md, "Surface
//! against surface"). No window opens.
//!
//! - `--surface-battle --survey [THEATER ...] [--surface-seed N] [--seeds
//!   N]` builds every theater's base layout, then each offered ground target
//!   at heavy defenses with seed N (1 by default; with `--seeds`, every seed
//!   from 1 to N), and lists every pair of opposing units where one may aim
//!   at the other and stands inside its weapon's launch range, with whether
//!   it can see it over the terrain. One `surface-battle: survey` line per
//!   build (with the nearest opposing pair out of reach), then a `pair` line
//!   per pair.
//! - `--surface-battle THEATER [STEM] [--surface-seed N] [--defenses AAA SAM]
//!   [--seconds S] [--blue ORDINAL,...] [--twice]` runs that mission's
//!   whole tick for S seconds
//!   (300 by default) with the player parked at a friendly field (a ground
//!   start: no unit shoots at an aircraft on the ground), and prints what
//!   the surface does to itself: each controller's first engagement of a
//!   ground unit or ship, every shot at one, every hit with the unit it
//!   credits, every kill, and every routed unit that halts to fight and
//!   drives on. It ends with a `summary` line and a digest of the end state
//!   (every combat row's hit points, every unit's controllers, magazines,
//!   rails and pose, and the surface's random draws). `--twice` runs it again
//!   from the start and prints whether both runs ended the same. `--blue`
//!   is the test mission: it hands the named template objects (by their
//!   ordinal in the template) to Blue before the first tick, so a Blue
//!   column stands among Red armour; retail never puts opposing units in
//!   reach of each other in a Quick Mission (see the survey).
//!
//! Like `--surface-dump` it adds the ground target templates and unit types
//! the import does not keep yet from the retail media, at runtime.
use crate::{AppResult, surface_dump};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::{aircraft::AircraftId, quick_template::tables};
use tore_sim::combat::live::{NO_SIDE, opposed};
use tore_world::{
    ai_wings::{ENEMY_SIDE, FRIENDLY_SIDE},
    mission::{Defense, MissionSpec, Start},
    seats::{SeatId, SeatInput},
    surface::{
        Surface, UnitId,
        fire::{self, Trace},
        movement,
    },
    world::{Seating, TickOutput, World},
};

const SEAT: SeatId = SeatId(0);

struct Options {
    positional: Vec<String>,
    seed: u32,
    defenses: (Defense, Defense),
    seconds: f64,
    twice: bool,
    survey: bool,
    seeds: Option<u32>,
    blue: Vec<u32>,
}

pub fn run() -> AppResult<()> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let mut o = Options {
        positional: Vec::new(),
        seed: 1,
        defenses: (Defense::Heavy, Defense::Heavy),
        seconds: 300.,
        twice: false,
        survey: false,
        seeds: None,
        blue: Vec::new(),
    };
    let level = |text: String| -> AppResult<Defense> {
        let n: usize = text.parse()?;
        Defense::ALL
            .get(n)
            .copied()
            .ok_or_else(|| format!("defense level {n}: 0 to 3").into())
    };
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let mut next = || it.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--surface-seed" => o.seed = next()?.parse()?,
            "--seconds" => o.seconds = next()?.parse()?,
            "--defenses" => {
                let aaa = level(next()?)?;
                let sam = level(next()?)?;
                o.defenses = (aaa, sam);
            }
            "--twice" => o.twice = true,
            "--seeds" => o.seeds = Some(next()?.parse()?),
            "--blue" => {
                for ordinal in next()?.split(',') {
                    o.blue.push(ordinal.parse()?);
                }
            }
            "--survey" => o.survey = true,
            other if other.starts_with("--") => return Err(format!("unknown {other}").into()),
            _ => o.positional.push(arg.clone()),
        }
    }
    let mut resources = crate::reel::load_assets()?.theater_resources;
    let added = surface_dump::with_retail_surface(&mut resources)?;
    println!("surface-battle: {added} resources added from the retail media");
    if o.survey {
        return survey(&resources, &o);
    }
    let (theater, stem) = match o.positional.as_slice() {
        [theater] => (theater.clone(), None),
        [theater, stem] => (theater.clone(), Some(stem.clone())),
        _ => {
            return Err("--surface-battle THEATER [STEM] [--surface-seed N] [--defenses AAA SAM] [--seconds S] [--blue ORDINAL,...] [--twice] | --survey [THEATER ...] [--seeds N]".into());
        }
    };
    let first = battle(&resources, &theater, stem.as_deref(), &o, true)?;
    if o.twice {
        let second = battle(&resources, &theater, stem.as_deref(), &o, false)?;
        println!(
            "surface-battle: repeat {} digest {first:#018x} then {second:#018x}",
            if first == second { "same" } else { "DIFFERENT" }
        );
        if first != second {
            return Err("the two runs ended differently".into());
        }
    }
    Ok(())
}

/// The mission of `theater` with the ground target `stem`, the player
/// parked at a friendly field.
fn spec(theater: &str, stem: Option<&str>, o: &Options, parked: bool) -> MissionSpec {
    let mut spec = MissionSpec::new(theater, AircraftId::F18);
    if let Some(stem) = stem {
        spec.ground_target = Some(stem.trim_start_matches('~').to_ascii_uppercase());
        spec.aaa = o.defenses.0;
        spec.sam = o.defenses.1;
    }
    spec.surface_seed = o.seed;
    // Airborne at 10,000 ft clears every theater's hills (5,000 ft does not
    // in Cuba); parked, no unit shoots at the player.
    spec.start = if parked {
        Start::GroundAuto {
            altitude_ft: 10_000,
        }
    } else {
        Start::Airborne {
            altitude_ft: 10_000,
        }
    };
    spec
}

/// The world for `spec`; a mission whose player cannot be parked (no
/// friendly field) starts airborne instead.
fn build(
    resources: &BTreeMap<String, Vec<u8>>,
    theater: &str,
    stem: Option<&str>,
    o: &Options,
) -> AppResult<(World, bool)> {
    match World::new(
        &spec(theater, stem, o, true),
        resources,
        Seating::SinglePlayer,
    ) {
        Ok(world) => Ok((world, true)),
        Err(_) => Ok((
            World::new(
                &spec(theater, stem, o, false),
                resources,
                Seating::SinglePlayer,
            )?,
            false,
        )),
    }
}

fn side(side: tore_sim::combat::live::Side) -> &'static str {
    if side == FRIENDLY_SIDE {
        "blue"
    } else if side == ENEMY_SIDE {
        "red"
    } else if side == NO_SIDE {
        "none"
    } else {
        "other"
    }
}

/// "T-72 #5 red" for a unit, its id for anything else.
fn name(surface: &Surface, id: u32) -> String {
    match surface.unit(UnitId(id)) {
        Some(unit) => format!("{} {:#010x} {}", unit.name, id, side(unit.side)),
        None => format!("{id:#010x}"),
    }
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// `--survey`: see the module comment.
fn survey(resources: &BTreeMap<String, Vec<u8>>, o: &Options) -> AppResult<()> {
    let mut builds = 0usize;
    let mut with_pairs = 0usize;
    let seeds: Vec<u32> = match o.seeds {
        Some(n) => (1..=n).collect(),
        None => vec![o.seed],
    };
    for seed in seeds {
        let o = &Options {
            positional: o.positional.clone(),
            seed,
            blue: Vec::new(),
            seeds: None,
            ..*o
        };
        for (index, theater) in tables::THEATERS.iter().enumerate() {
            if !o.positional.is_empty()
                && !o.positional.iter().any(|w| w.eq_ignore_ascii_case(theater))
            {
                continue;
            }
            let stems = std::iter::once(None).chain(
                tables::TEMPLATES[index]
                    .iter()
                    .filter(|stem| !stem.ends_with("NOTH"))
                    .map(|stem| Some(*stem)),
            );
            for stem in stems {
                let world = match World::new(
                    &spec(theater, stem, o, false),
                    resources,
                    Seating::SinglePlayer,
                ) {
                    Ok(world) => world,
                    Err(error) => {
                        println!(
                            "surface-battle: survey {theater} {} error {error}",
                            stem.unwrap_or("base")
                        );
                        continue;
                    }
                };
                builds += 1;
                NEAREST.set(f64::INFINITY);
                let pairs = pairs(&world);
                let nearest = NEAREST.get();
                let seen = pairs.iter().filter(|p| p.seen).count();
                if !pairs.is_empty() {
                    with_pairs += 1;
                }
                let surface = &world.terrain.surface;
                let contacts = fire::surface_contacts(surface, &world.combat.state);
                let count = |s| contacts.iter().filter(|c| c.side == s).count();
                println!(
                    "surface-battle: survey {theater} {} seed {} units {} red {} blue {} shooters {} pairs {} seen {} nearest-out-of-reach {}",
                    stem.unwrap_or("base"),
                    o.seed,
                    surface.units.len(),
                    count(ENEMY_SIDE),
                    count(FRIENDLY_SIDE),
                    surface
                        .arsenal
                        .units
                        .iter()
                        .filter(|arms| arms.weapons.iter().any(|w| w.aims_at_any_surface()))
                        .count(),
                    pairs.len(),
                    seen,
                    if nearest.is_finite() {
                        format!("{nearest:.0} ft")
                    } else {
                        "none".to_string()
                    }
                );
                for pair in &pairs {
                    println!(
                        "surface-battle: pair {theater} {} {} -> {} with {} range {:.0} ft sight {}",
                        stem.unwrap_or("base"),
                        name(surface, pair.shooter),
                        name(surface, pair.target),
                        pair.weapon,
                        pair.range,
                        if pair.seen { "yes" } else { "no" }
                    );
                }
            }
        }
    }
    println!("surface-battle: survey done builds {builds} with-pairs {with_pairs}");
    Ok(())
}

thread_local! {
    /// The nearest opposing pair out of reach the last [`pairs`] call met,
    /// feet: the survey's measure of how far apart the sides stand.
    static NEAREST: std::cell::Cell<f64> = const { std::cell::Cell::new(f64::INFINITY) };
}

struct Pair {
    shooter: u32,
    target: u32,
    weapon: String,
    range: f64,
    seen: bool,
}

/// Every opposing pair in `world` as it starts: a unit with a weapon that
/// may aim at the other, which stands inside that weapon's launch range.
fn pairs(world: &World) -> Vec<Pair> {
    let surface = &world.terrain.surface;
    let terrain = &world.terrain;
    let ground = |x: f64, z: f64| f64::from(terrain.height(x as f32, z as f32));
    let contacts = fire::surface_contacts(surface, &world.combat.state);
    let mut out = Vec::new();
    for arms in &surface.arsenal.units {
        let eye = arms.place(None).eye;
        for weapon in arms.weapons.iter().filter(|w| w.aims_at_any_surface()) {
            for contact in &contacts {
                if !opposed(arms.side, contact.side) || !weapon.aims_at_surface(contact.category) {
                    continue;
                }
                let range = distance(eye, contact.position);
                if range > weapon.launch.max_range {
                    if range < NEAREST.get() {
                        NEAREST.set(range);
                    }
                    continue;
                }
                out.push(Pair {
                    shooter: arms.unit.0,
                    target: contact.id,
                    weapon: weapon.record.source.clone(),
                    range,
                    seen: fire::line_of_sight(eye, contact.position, &ground),
                });
            }
        }
    }
    out
}

/// Hands template unit `id` to Blue in every place the world keeps its side:
/// the resolved unit, the scene's side table, its arms and its combat row.
/// A test aid (the surface digest no longer matches the mission text, which
/// only a networked game checks).
pub(crate) fn hand_to_blue(world: &mut World, id: UnitId) -> AppResult<()> {
    let surface = &mut world.terrain.surface;
    let unit = surface
        .units
        .iter_mut()
        .find(|unit| unit.id == id)
        .ok_or_else(|| format!("--blue: no template object {:#010x}", id.0))?;
    unit.side = FRIENDLY_SIDE;
    surface.object_sides.insert(id.0, FRIENDLY_SIDE);
    if let Some(arms) = surface
        .arsenal
        .units
        .iter_mut()
        .find(|arms| arms.unit == id)
    {
        arms.side = FRIENDLY_SIDE;
    }
    if let Some(row) = world.combat.state.targets.iter_mut().find(|t| t.id == id.0) {
        row.side = FRIENDLY_SIDE;
    }
    Ok(())
}

/// FNV-1a 64 over the end state.
#[derive(Default)]
struct Digest(u64);
impl Digest {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, data: &[u8]) {
        for byte in data {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }
}

fn digest(world: &World) -> u64 {
    let mut h = Digest::new();
    let mut rows: Vec<(u32, i32)> = world
        .combat
        .state
        .targets
        .iter()
        .map(|t| (t.id, t.hp))
        .collect();
    rows.sort_unstable();
    for (id, hp) in rows {
        h.u64(u64::from(id));
        h.u64(hp as u64);
    }
    let state = &world.combat.surface;
    h.u64(state.rng);
    for unit in &state.units {
        h.u64(u64::from(unit.id.0));
        for engager in &unit.engagers {
            h.bytes(format!("{:?}", engager.controller.phase()).as_bytes());
            h.u64(u64::from(engager.controller.target().unwrap_or(0)));
        }
        for mount in &unit.mounts {
            h.u64(u64::from(mount.loaded));
            h.u64(u64::from(mount.reserve.unwrap_or(u32::MAX)));
        }
        if let Some(mover) = unit.mover {
            h.u64(mover.x as u64);
            h.u64(mover.y as u64);
            h.u64(mover.z as u64);
            h.u64(mover.speed as u64);
        }
    }
    h.0
}

/// One run: see the module comment. Prints when `loud`; returns the end
/// state's digest.
fn battle(
    resources: &BTreeMap<String, Vec<u8>>,
    theater: &str,
    stem: Option<&str>,
    o: &Options,
    loud: bool,
) -> AppResult<u64> {
    let (mut world, parked) = build(resources, theater, stem, o)?;
    if let Some(why) = &world.terrain.surface.unresolved {
        return Err(format!("the ground target stands nowhere: {why}").into());
    }
    for ordinal in &o.blue {
        let id = UnitId::template(*ordinal).ok_or("--blue: not a template ordinal")?;
        hand_to_blue(&mut world, id)?;
    }
    let say = |line: String| {
        if loud {
            println!("surface-battle: {line}");
        }
    };
    let pairs = pairs(&world);
    say(format!(
        "start {theater} {} seed {} player {} pairs {} seen {}",
        stem.unwrap_or("base"),
        o.seed,
        if parked { "parked" } else { "airborne" },
        pairs.len(),
        pairs.iter().filter(|p| p.seen).count()
    ));
    let surface_ids: BTreeSet<u32> = world.terrain.surface.units.iter().map(|u| u.id.0).collect();
    let hp = |world: &World| -> BTreeMap<u32, i32> {
        world
            .combat
            .state
            .targets
            .iter()
            .filter(|t| surface_ids.contains(&t.id))
            .map(|t| (t.id, t.hp))
            .collect()
    };
    let mut before = hp(&world);
    let mut engaged: BTreeSet<(u32, usize)> = BTreeSet::new();
    let mut halted: BTreeSet<u32> = BTreeSet::new();
    let (mut shots, mut rounds, mut hits, mut kills, mut halts) = (0u32, 0u32, 0u32, 0u32, 0u32);
    let mut first_shot: Option<f64> = None;
    let mut out = TickOutput::default();
    let ticks = (o.seconds * 120.).round() as u64;
    for tick in 0..ticks {
        let input = SeatInput {
            seat: SEAT,
            tick: world.tick(),
            ..SeatInput::default()
        };
        world.step(std::slice::from_ref(&input), &mut out)?;
        let t = (tick + 1) as f64 / 120.;
        let surface = &world.terrain.surface;
        for trace in &world.combat.surface.trace {
            match trace {
                Trace::Phase {
                    unit,
                    weapon,
                    target: Some(target),
                    ..
                } if fire::is_surface_id(*target) && engaged.insert((unit.0, *weapon)) => {
                    say(format!(
                        "t {t:.1} engage {} weapon {weapon} -> {}",
                        name(surface, unit.0),
                        name(surface, *target)
                    ));
                }
                Trace::Shot {
                    unit,
                    record,
                    target,
                    rounds: fired,
                    ..
                } if fire::is_surface_id(*target) && *fired > 0 => {
                    shots += 1;
                    rounds += fired;
                    first_shot.get_or_insert(t);
                    say(format!(
                        "t {t:.1} shot {} {record} x{fired} -> {}",
                        name(surface, unit.0),
                        name(surface, *target)
                    ));
                }
                _ => {}
            }
        }
        let now = hp(&world);
        for (id, hp_now) in &now {
            let was = before.get(id).copied().unwrap_or(*hp_now);
            if *hp_now < was {
                let by = world.combat.state.ledger.credit(*id).map(|kill| kill.owner);
                hits += 1;
                say(format!(
                    "t {t:.1} hit {} {was} -> {hp_now} by {}",
                    name(surface, *id),
                    by.map_or_else(|| "nobody".into(), |by| name(surface, by))
                ));
                if *hp_now <= 0 {
                    kills += 1;
                    say(format!(
                        "t {t:.1} kill {} by {}",
                        name(surface, *id),
                        by.map_or_else(|| "nobody".into(), |by| name(surface, by))
                    ));
                }
            }
        }
        before = now;
        for id in surface.courses.keys() {
            let Some(state) = world.combat.surface.unit(*id) else {
                continue;
            };
            let alive = before.get(&id.0).is_some_and(|hp| *hp > 0);
            let holding = movement::engaging_surface(state) && alive;
            if holding && halted.insert(id.0) {
                halts += 1;
                say(format!("t {t:.1} halt {} to fight", name(surface, id.0)));
            } else if !holding && halted.remove(&id.0) && alive {
                say(format!("t {t:.1} drive-on {}", name(surface, id.0)));
            }
        }
    }
    let end = digest(&world);
    say(format!(
        "summary engagements {} shots {shots} rounds {rounds} hits {hits} kills {kills} halts {halts} first-shot {} digest {end:#018x}",
        engaged.len(),
        first_shot.map_or_else(|| "none".into(), |t| format!("{t:.1}")),
    ));
    Ok(end)
}
