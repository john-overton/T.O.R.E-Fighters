//! `--surface-objective THEATER STEM [--surface-seed N] [--defenses AAA SAM]
//! [--redfor] [--from NM] [--altitude FT] [--seconds S] [--kill-friendly]
//! [--record PATH [--verify-render]] [--run-on] [--vulnerable] [--no-relocate]
//! [--no-jitter] [--shuttle] [--follow-terrain]`: a
//! development aid and the battery's check of the ground target's objectives
//! (docs/spec/surface-defenses.md, "Objectives, scoring and debrief"). No
//! window opens; the whole mission tick runs as in the game.
//!
//! The player flies a scripted straight line at the ground target's
//! defenses, invulnerable, so its SAMs and guns fire at it and the debrief
//! counts their fire. When it is over the middle of the site, a Mk 82 is
//! placed on every ground target (the player's, in Blue's mission) every two
//! seconds until it is gone. The run prints the objectives and the debrief's
//! tallies before and after, each line starting `surface-objective:`.
//!
//! `--redfor` builds the multiplayer mission instead (open seating) and seats
//! the human in the first plane of the enemy wing: Redfor defends the target,
//! so the objective is the Protect line, and the bombs are a Blue plane's.
//! `--kill-friendly` also destroys one friendly unit that is not a target,
//! which fails the mission (retail's friendly fire).
//!
//! `--record PATH` records the run as a mission recording (a format 3 file for
//! a ground target), `--verify-render` reads it back and checks that every
//! recorded tick redraws the picture the run drew, and `--run-on` keeps the
//! run going to `--seconds` after every target is down, so trucks can rearm
//! the launchers the pass emptied. The battery's `replay-surface` scenario
//! uses them. `--vulnerable` takes the invulnerability away, so the defenses
//! can shoot the player down.
//!
//! `--no-relocate` (and `--no-jitter`) fly the template at its retail spot,
//! as the preview tools show it. `--shuttle` flies the line back and forth
//! across the site for the whole run instead of once, so the defenses keep
//! firing, empty their rails and the trucks rearm them. `--follow-terrain`
//! holds `--altitude` above the ground under the aircraft, never below the
//! site's own height plus `--altitude`, so a line over hills does not fly
//! into them (a crashed player is ignored by the defenses). With `--run-on` it is
//! the per-theater acceptance run of docs/baselines/surface-defenses.md. The
//! run ends with a `surface-objective: summary` line: the template's units,
//! batteries and trucks, the shots by kind (template units and base-layout
//! units apart), rearms and refills, kills and the times of the first shot,
//! the first missile and the last target's fall, and when the player crashed.
use crate::{AppResult, replay, snapshot::RenderSnapshot, surface_dump};
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::{
    attitude::Vector,
    combat::{FallState, ledger::ShotKind, live::Projectile},
};
use tore_world::{
    ai_wings::{ENEMY_SIDE, FRIENDLY_SIDE},
    debrief,
    mission::{Defense, MissionSpec, Skill, WingSpec},
    seats::{SeatId, SeatInput},
    surface::{IdRange, Surface, UnitId, fire::Trace},
    world::{Hooks, Seating, TickOutput, World},
};

const SEAT: SeatId = SeatId(0);
const PLAYER: u32 = 0;
/// Ids for the bombs the run places, clear of the ownships' and the AI's.
const BOMB_ID_BASE: u32 = 0x00F0_0000;
const FEET_PER_NM: f64 = 6_076.;
const FPS_PER_KNOT: f64 = 1.687_8;

struct Options {
    theater: String,
    stem: String,
    seed: u32,
    defenses: (usize, usize),
    redfor: bool,
    from_nm: f64,
    altitude: f64,
    seconds: f64,
    kill_friendly: bool,
    record: Option<std::path::PathBuf>,
    verify: bool,
    run_on: bool,
    vulnerable: bool,
    no_relocate: bool,
    no_jitter: bool,
    shuttle: bool,
    follow_terrain: bool,
}

fn options() -> AppResult<Options> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let mut o = Options {
        theater: String::new(),
        stem: String::new(),
        seed: 1,
        defenses: (3, 3),
        redfor: false,
        from_nm: 10.,
        altitude: 3_000.,
        seconds: 200.,
        kill_friendly: false,
        record: None,
        verify: false,
        run_on: false,
        vulnerable: false,
        no_relocate: false,
        no_jitter: false,
        shuttle: false,
        follow_terrain: false,
    };
    let mut positional = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut next = || -> AppResult<String> {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value").into())
        };
        match arg.as_str() {
            "--surface-seed" => o.seed = next()?.parse()?,
            "--defenses" => o.defenses = (next()?.parse()?, next()?.parse()?),
            "--redfor" => o.redfor = true,
            "--from" => o.from_nm = next()?.parse()?,
            "--altitude" => o.altitude = next()?.parse()?,
            "--seconds" => o.seconds = next()?.parse()?,
            "--kill-friendly" => o.kill_friendly = true,
            "--record" => o.record = Some(next()?.into()),
            "--verify-render" => o.verify = true,
            "--run-on" => o.run_on = true,
            "--vulnerable" => o.vulnerable = true,
            "--no-relocate" => o.no_relocate = true,
            "--no-jitter" => o.no_jitter = true,
            "--shuttle" => o.shuttle = true,
            "--follow-terrain" => o.follow_terrain = true,
            other if other.starts_with("--") => return Err(format!("unknown {other}").into()),
            other => positional.push(other.to_owned()),
        }
    }
    let [theater, stem] = positional.as_slice() else {
        return Err("--surface-objective THEATER STEM [--surface-seed N] [--defenses AAA SAM] [--redfor] [--from NM] [--altitude FT] [--seconds S] [--kill-friendly] [--record PATH [--verify-render]] [--run-on] [--vulnerable] [--no-relocate] [--no-jitter] [--shuttle] [--follow-terrain]".into());
    };
    o.theater = theater.to_ascii_uppercase();
    o.stem = stem.trim_start_matches('~').to_ascii_uppercase();
    if o.verify && o.record.is_none() {
        return Err("--verify-render checks a --record run".into());
    }
    Ok(o)
}

fn spec(o: &Options) -> AppResult<MissionSpec> {
    let mut spec = MissionSpec::new(&o.theater, AircraftId::F18);
    spec.ground_target = Some(o.stem.clone());
    spec.aaa = Defense::from_level(o.defenses.0).ok_or("defense levels are 0 to 3")?;
    spec.sam = Defense::from_level(o.defenses.1).ok_or("defense levels are 0 to 3")?;
    spec.surface_seed = o.seed;
    if !o.vulnerable {
        spec.cheats.damage = tore_sim::cheats::Damage::Invulnerable;
    }
    if o.redfor {
        // A multiplayer mission: two Blue and two Redfor aircraft, the AI
        // flying them until a human takes one.
        for index in [0, 3] {
            spec.wings[index] = WingSpec {
                aircraft: AircraftId::F18,
                count: 2,
                skill: Skill::Average,
            };
        }
    }
    spec.validate()?;
    Ok(spec)
}

fn distance(a: Vector, b: Vector) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn line(t: f64, text: impl std::fmt::Display) {
    println!("surface-objective: t={t:.1} {text}");
}

/// The report as the lines the battery reads.
fn print(label: &str, report: &debrief::Report) {
    let objectives: Vec<String> = report.objectives.iter().map(|o| o.sentence()).collect();
    println!(
        "surface-objective: {label} outcome {} objectives {:?}",
        report.outcome.label(),
        objectives
    );
    let tally = |t: tore_sim::combat::ledger::Tally| format!("{}/{}", t.hit, t.launched);
    let player = &report.player;
    println!(
        "surface-objective: {label} enemy-sam {} enemy-aaa {} enemy-aam {} enemy-gun {} bombs {} kills {:?} friendly-fire {} shot-down-by {}",
        tally(player.enemy_sam),
        tally(player.enemy_aaa),
        tally(player.enemy_aam),
        tally(player.enemy_gun),
        tally(player.bombs),
        player.kills,
        player.friendly_fire,
        player.shot_down_by.as_deref().unwrap_or("-"),
    );
}

pub fn run() -> AppResult<()> {
    let o = options()?;
    let mut resources = crate::reel::load_assets()?.theater_resources;
    surface_dump::with_retail_surface(&mut resources)?;
    let spec = spec(&o)?;
    let seating = if o.redfor {
        Seating::Open
    } else {
        Seating::SinglePlayer
    };
    let variation = tore_world::surface::layout::Variation {
        jitter: !o.no_jitter,
        relocate: !o.no_relocate,
    };
    let mut world = World::build(
        &spec,
        &resources,
        seating,
        &mut Hooks {
            ground_variation: Some(variation),
            ..Hooks::default()
        },
    )?
    .world;
    if let Some(why) = &world.terrain.surface.unresolved {
        return Err(format!("the ground target stands nowhere: {why}").into());
    }
    // Who the human is and who drops the bombs.
    let (owner, side) = if o.redfor {
        let enemy_lead = world
            .roster
            .planes()
            .iter()
            .filter(|p| p.slot.wing.side.is_enemy() && p.slot.member == 0)
            .map(|p| p.id)
            .min()
            .ok_or("no enemy aircraft to seat the Redfor human in")?;
        let blue = world
            .roster
            .planes()
            .iter()
            .filter(|p| !p.slot.wing.side.is_enemy())
            .map(|p| p.id)
            .min()
            .ok_or("no Blue aircraft to drop the bombs")?;
        world.take_plane(SEAT, enemy_lead)?;
        (blue.0, ENEMY_SIDE)
    } else {
        (PLAYER, FRIENDLY_SIDE)
    };
    let plane = world
        .roster
        .seat(SEAT)
        .and_then(|seat| seat.plane)
        .ok_or("the seat flies no plane")?
        .0;
    let surface = world.terrain.surface.clone();
    let targets = tore_world::ai_wings::outcome::ground_targets(&surface, &world.combat.state);
    println!(
        "surface-objective: side {} plane {plane} targets {} units {} parked {}",
        if o.redfor { "redfor" } else { "blue" },
        targets.len(),
        surface.units.len(),
        surface.parked_scene.len()
    );
    for id in &targets {
        if let Some(unit) = surface.unit(tore_world::surface::UnitId(*id)) {
            println!(
                "surface-objective: target {id:#010x} {} class {:#06x} side {}",
                unit.resource, unit.class, unit.side.0
            );
        } else if let Some(pose) = surface.parked_scene.iter().find(|p| p.id.0 == *id) {
            println!(
                "surface-objective: target {id:#010x} {} class {:#06x} side {}",
                pose.resource, pose.class, pose.side.0
            );
        }
    }
    let mut tally = Tally::default();
    print_layout(&surface);
    let first = capture(&world)?;
    print("start", &first);
    let mut record = o
        .record
        .as_ref()
        .map(|path| Record::start(&world, path, o.verify))
        .transpose()?;

    // The line the human flies: at the middle of the targets, `--from` nm out.
    let center = {
        let rows: Vec<Vector> = world
            .combat
            .state
            .targets
            .iter()
            .filter(|t| targets.contains(&t.id))
            .map(|t| t.position)
            .collect();
        if rows.is_empty() {
            return Err("the mission has no ground target rows".into());
        }
        std::array::from_fn::<f64, 3, _>(|i| {
            rows.iter().map(|r| r[i]).sum::<f64>() / rows.len() as f64
        })
    };
    let heading = 0f64;
    let forward = [heading.sin(), 0., heading.cos()];
    let speed = 400. * FPS_PER_KNOT;
    let start: Vector = std::array::from_fn(|i| center[i] - forward[i] * o.from_nm * FEET_PER_NM);
    let ground = world.terrain.height(center[0] as f32, center[2] as f32);
    let altitude = f64::from(ground) + o.altitude;
    let over_at = o.from_nm * FEET_PER_NM / speed;
    let bomb = {
        let name = "MK82.JT";
        let bytes = resources.get(name).ok_or("missing MK82.JT")?;
        Weapon::parse(name, bytes)?
    };
    let friendly = o
        .kill_friendly
        .then(|| {
            surface
                .units
                .iter()
                .find(|u| u.side == side && !u.is_target() && world_has_row(&world, u.id.0))
                .map(|u| u.id.0)
        })
        .flatten();
    if o.kill_friendly && friendly.is_none() {
        return Err("no friendly unit that is not a target to destroy".into());
    }
    let mut out = TickOutput::default();
    let mut next_bomb = BOMB_ID_BASE;
    let mut next_strike = over_at;
    for tick in 0..(o.seconds * 120.) as u64 {
        let t = tick as f64 / 120.;
        {
            // Along the line, and back again when shuttling.
            let leg = 2. * o.from_nm * FEET_PER_NM;
            let flown = speed * t;
            let (along, sign) = if o.shuttle && flown % (2. * leg) >= leg {
                (2. * leg - flown % (2. * leg), -1.)
            } else if o.shuttle {
                (flown % (2. * leg), 1.)
            } else {
                (flown, 1.)
            };
            let x = start[0] + forward[0] * along;
            let z = start[2] + forward[2] * along;
            let y = if o.follow_terrain {
                altitude.max(f64::from(world.terrain.height(x as f32, z as f32)) + o.altitude)
            } else {
                altitude
            };
            let flight = &mut world.cockpits[0].flight;
            flight.position = [x, y, z];
            flight.velocity = [forward[0] * speed * sign, 0., forward[2] * speed * sign];
            flight.speed = speed;
            flight.yaw = if sign > 0. {
                heading
            } else {
                heading + std::f64::consts::PI
            };
            flight.pitch = 0.;
            flight.bank = 0.;
            flight.vertical_speed = 0.;
            flight.roll_rate = 0.;
            flight.pitch_rate = 0.;
        }
        if t >= next_strike {
            next_strike += 2.;
            let mut wanted: Vec<u32> = targets.clone();
            wanted.extend(friendly);
            for id in wanted {
                let Some(row) = world
                    .combat
                    .state
                    .targets
                    .iter()
                    .find(|row| row.id == id && row.hp > 0)
                else {
                    continue;
                };
                // Dropped 150 ft above the aim point, ahead of a moving unit
                // by the 0.3 s the bomb takes to fall.
                let from = std::array::from_fn(|i| {
                    row.position[i] - row.velocity[i] * 0.3 + [0., 150., 0.][i]
                });
                let state = &mut world.combat.state;
                state
                    .ledger
                    .launch(next_bomb, owner, Some(id), ShotKind::Bomb);
                state.projectiles.push(Projectile {
                    id: next_bomb,
                    owner,
                    weapon: Some(bomb.clone()),
                    guidance: None,
                    motion: None,
                    guidance_ticks: None,
                    age: 0,
                    incoming: None,
                    station: 0,
                    position: from,
                    previous: from,
                    direction: [0., -1., 0.],
                    speed_f8: (500. * 256.) as i32,
                    launched_t: (state.tick() / 30) as u16,
                    target: None,
                    fall: FallState::default(),
                    gun_round: None,
                    tracer: false,
                });
                next_bomb += 1;
            }
            line(t, "bombs placed");
        }
        let input = SeatInput {
            seat: SEAT,
            tick: world.tick(),
            ..SeatInput::default()
        };
        if let Some(record) = &mut record {
            record.before(&mut world);
        }
        world.step(std::slice::from_ref(&input), &mut out)?;
        if let Some(record) = &mut record {
            record.after(&mut world, &out);
        }
        tally.tick(&world, t, &targets);
        // Stop when every target has fallen and the friendly one too.
        if !o.run_on
            && t > over_at + 4.
            && targets.iter().all(|id| row_dead(&world, *id))
            && friendly.is_none_or(|id| row_dead(&world, id))
        {
            line(t, "every target is down");
            break;
        }
        if tick % (120 * 20) == 0 {
            let near = world
                .combat
                .state
                .targets
                .iter()
                .filter(|row| targets.contains(&row.id) && row.hp > 0)
                .map(|row| distance(row.position, world.cockpits[0].flight.position))
                .fold(f64::INFINITY, f64::min);
            if near.is_finite() {
                line(t, format_args!("nearest living target {near:.0} ft"));
            } else {
                line(t, "no target is left standing");
            }
        }
    }
    // Let the last bombs land and the world settle.
    for _ in 0..240 {
        let input = SeatInput {
            seat: SEAT,
            tick: world.tick(),
            ..SeatInput::default()
        };
        if let Some(record) = &mut record {
            record.before(&mut world);
        }
        world.step(std::slice::from_ref(&input), &mut out)?;
        if let Some(record) = &mut record {
            record.after(&mut world, &out);
        }
        let t = world.tick() as f64 / 120.;
        tally.tick(&world, t, &targets);
    }
    for id in &targets {
        if let Some(row) = world.combat.state.targets.iter().find(|row| row.id == *id) {
            println!(
                "surface-objective: end target {id:#010x} hp {}/{}",
                row.hp, row.initial_hp
            );
        }
    }
    let last = capture(&world)?;
    print("end", &last);
    tally.print(&world, &surface, &targets, &last);
    for objective in &last.objectives {
        println!("surface-objective: sentence {}", objective.sentence());
    }
    if let Some(record) = record {
        for line in record.finish(&world, &last)? {
            println!("surface-objective: {line}");
        }
    }
    println!("surface-objective: done");
    Ok(())
}

/// The run as a mission recording, the way the AI probe records: the picture
/// is taken as live flight takes it and nothing it reads feeds back.
struct Record {
    recorder: replay::recorder::Recorder,
    /// Every tick's picture, when the run checks the recording against them.
    pictures: Option<Vec<RenderSnapshot>>,
    devices: Vec<(u64, u64)>,
}

impl Record {
    fn start(world: &World, path: &std::path::Path, verify: bool) -> AppResult<Self> {
        use replay::{convert, recorder};
        let combat = &world.combat;
        let flight = &world.cockpits[0].flight;
        let snapshot = combat.render_snapshot();
        let extra = vec![(
            "probe".to_owned(),
            "the surface objective run on the full mission tick: no audio, music, HUD or rumble"
                .to_owned(),
        )];
        let header = recorder::header(
            tore_replay::MissionKind::Probe,
            &world.terrain,
            &convert::Presentation::of(snapshot),
            extra,
            std::time::SystemTime::now(),
        );
        let player = recorder::Human::single_player("F/A-18D", true);
        let roster = recorder::roster(
            snapshot,
            &player,
            &[],
            world.ai_wings.as_ref(),
            combat.dummy_types(),
        );
        let mut recorder = recorder::Recorder::start(path.to_path_buf(), &header, &roster)
            .map_err(|error| format!("--record {}: {error}", path.display()))?
            .for_seat(SEAT, player.id);
        recorder.wait_for_writer();
        recorder.begin(recorder::Tick {
            snapshot,
            combat,
            flight,
            previous: flight,
            pilot: &tore_sim::flight::PilotInput::default(),
            others: &[],
            wings: world.ai_wings.as_ref(),
            world: &world.terrain,
            events: &[],
            outcomes: &[],
            journal: None,
        });
        Ok(Self {
            recorder,
            pictures: verify.then(|| vec![snapshot.clone()]),
            devices: Vec::new(),
        })
    }

    fn before(&mut self, world: &mut World) {
        self.recorder.start_tick(None, &mut world.combat);
    }

    fn after(&mut self, world: &mut World, out: &TickOutput) {
        let idle = tore_sim::flight::PilotInput::default();
        let cockpit = &world.cockpits[0];
        self.recorder.begin(replay::recorder::Tick {
            snapshot: world.combat.render_snapshot(),
            combat: &world.combat,
            flight: &cockpit.flight,
            previous: &cockpit.previous_flight,
            pilot: &idle,
            others: &[],
            wings: world.ai_wings.as_ref(),
            world: &world.terrain,
            events: &out.events,
            outcomes: &out.outcomes,
            journal: out.journal.as_ref(),
        });
        self.recorder.end(None, &mut world.combat);
        if let Some(pictures) = &mut self.pictures {
            pictures.push(world.combat.render_snapshot().clone());
        }
        self.devices.push((
            world.combat.state.tick(),
            replay::devices::digest(&world.combat.state.devices),
        ));
    }

    /// Finishes the file and, when asked, replays it against the run.
    fn finish(mut self, world: &World, report: &debrief::Report) -> AppResult<Vec<String>> {
        self.recorder.note(
            tore_replay::Event::new(tore_replay::vocab::kind::SYSTEM_END)
                .with(tore_replay::vocab::field::REASON, "the run finished"),
        );
        let footer = crate::replay_footer(&world.combat, PLAYER, Some(report), "the run finished");
        let path = self
            .recorder
            .finish(&footer)
            .ok_or("the recording could not be finished; see the session log")?;
        let mut lines = vec![format!("recorded {}", path.display())];
        if let Some(pictures) = &self.pictures {
            let verification = replay::cli::verify(&path, pictures, &self.devices, &world.terrain)?;
            lines.push(verification.line());
        }
        Ok(lines)
    }
}

fn capture(world: &World) -> AppResult<debrief::Report> {
    debrief::capture(world, SEAT).ok_or_else(|| "the seat flies no plane".into())
}

fn row_dead(world: &World, id: u32) -> bool {
    world
        .combat
        .state
        .targets
        .iter()
        .find(|row| row.id == id)
        .is_none_or(|row| row.hp <= 0)
}

fn world_has_row(world: &World, id: u32) -> bool {
    world.combat.state.targets.iter().any(|row| row.id == id)
}

/// The template's units, batteries and trucks as the run flies them.
fn print_layout(surface: &Surface) {
    let template: Vec<_> = surface.template_units().collect();
    let class = |bit: u16| template.iter().filter(|u| u.class & bit != 0).count();
    let in_template = |id: UnitId| id.range() == IdRange::Template;
    let batteries: Vec<String> = surface
        .batteries
        .iter()
        .filter(|b| b.launchers.iter().any(|id| in_template(*id)))
        .map(|b| format!("{:?}", b.system))
        .collect();
    let trucks = surface
        .trucks
        .iter()
        .filter(|t| in_template(t.id) || t.serves.is_some_and(in_template))
        .count();
    let armed = template
        .iter()
        .filter(|u| surface.arsenal.arms(u.id).is_some())
        .count();
    let (anchor, relocate, moved) = surface.template.as_ref().map_or(("none", 0, 0.), |site| {
        let t = surface.transform.translation;
        (
            site.anchor
                .map_or("none", tore_world::surface::layout::Anchor::name),
            u8::from(site.settings.variation.relocate),
            f64::from(t[0]).hypot(f64::from(t[1])) / FEET_PER_NM,
        )
    });
    println!(
        "surface-objective: layout units {} armed {armed} sam {} aaa {} ships {} batteries {} [{}] trucks {trucks} parked {} anchor {anchor} relocate {relocate} moved-nm {moved:.1} layout-units {}",
        template.len(),
        class(0x1000),
        class(0x0800),
        class(0x2000),
        batteries.len(),
        batteries.join(","),
        surface.parked_scene.len(),
        surface.layout_units().count(),
    );
}

/// What the defenses did during the run, from the surface tick's trace.
#[derive(Default)]
struct Tally {
    /// (template unit, record) shots: rounds or missiles.
    missiles: [u32; 2],
    rounds: [u32; 2],
    flak: [u32; 2],
    refused: u32,
    by_record: std::collections::BTreeMap<String, u32>,
    rearms: u32,
    refills: u32,
    swaps: u32,
    first_shot: Option<f64>,
    first_missile: Option<f64>,
    all_down: Option<f64>,
    crashed: Option<f64>,
}

impl Tally {
    fn tick(&mut self, world: &World, t: f64, targets: &[u32]) {
        for line in &world.combat.surface.trace {
            match line {
                Trace::Shot {
                    unit,
                    record,
                    rounds,
                    refused,
                    flak,
                    ..
                } => {
                    let at = usize::from(
                        unit.range() != IdRange::Template
                            && unit.range() != IdRange::BatteryRadar
                            && unit.range() != IdRange::SupplyTruck,
                    );
                    let missile = !tore_sim::combat::surface_guns::is_surface_gun(record);
                    if missile {
                        self.missiles[at] += rounds;
                        self.first_missile.get_or_insert(t);
                    } else if *flak {
                        self.flak[at] += rounds;
                    } else {
                        self.rounds[at] += rounds;
                    }
                    if *rounds > 0 {
                        self.first_shot.get_or_insert(t);
                    }
                    self.refused += refused;
                    *self.by_record.entry(record.clone()).or_default() += rounds;
                }
                Trace::Rearm { .. } => self.rearms += 1,
                Trace::Refill { .. } => self.refills += 1,
                Trace::Swap { .. } => self.swaps += 1,
                _ => {}
            }
        }
        if self.crashed.is_none() && world.cockpits[0].flight.crashed {
            self.crashed = Some(t);
        }
        if self.all_down.is_none()
            && !targets.is_empty()
            && targets.iter().all(|id| row_dead(world, *id))
        {
            self.all_down = Some(t);
        }
    }

    fn print(&self, world: &World, surface: &Surface, targets: &[u32], report: &debrief::Report) {
        let down = targets.iter().filter(|id| row_dead(world, **id)).count();
        let template_dead = surface
            .template_units()
            .filter(|u| world_has_row(world, u.id.0) && row_dead(world, u.id.0))
            .count();
        let time = |v: Option<f64>| v.map_or("-".into(), |t| format!("{t:.1}"));
        let records: Vec<String> = self
            .by_record
            .iter()
            .map(|(record, n)| format!("{}:{n}", record.trim_end_matches(".JT")))
            .collect();
        println!(
            "surface-objective: summary outcome {} targets {down}/{} template-dead {template_dead} missiles {} rounds {} flak {} layout-missiles {} layout-rounds {} layout-flak {} refused {} swaps {} rearms {} refills {} first-shot-s {} first-missile-s {} all-down-s {} crashed-s {} end-s {:.1} sam-hits {}/{} aaa-hits {}/{} records [{}]",
            report.outcome.label(),
            targets.len(),
            self.missiles[0],
            self.rounds[0],
            self.flak[0],
            self.missiles[1],
            self.rounds[1],
            self.flak[1],
            self.refused,
            self.swaps,
            self.rearms,
            self.refills,
            time(self.first_shot),
            time(self.first_missile),
            time(self.all_down),
            time(self.crashed),
            world.tick() as f64 / 120.,
            report.player.enemy_sam.hit,
            report.player.enemy_sam.launched,
            report.player.enemy_aaa.hit,
            report.player.enemy_aaa.launched,
            records.join(","),
        );
    }
}
