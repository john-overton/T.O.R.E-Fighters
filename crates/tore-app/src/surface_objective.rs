//! `--surface-objective THEATER STEM [--surface-seed N] [--defenses AAA SAM]
//! [--redfor] [--from NM] [--altitude FT] [--seconds S] [--kill-friendly]
//! [--record PATH [--verify-render]] [--run-on] [--vulnerable]`: a
//! development aid and the battery's check of the ground target's objectives
//! (docs/spec/surface-defenses.md, "Objectives, scoring and debrief"). No
//! window opens; the whole mission tick runs as in the game.
//!
//! The player flies a scripted straight line at the ground target's
//! defenses, invulnerable, so its SAMs and guns fire at it and the debrief
//! counts their fire. When it is over the middle of the site, a Mk 82 is put
//! in the air for every ground target (the player's, in Blue's mission)
//! every two seconds until it is gone, as a 45 degree dive at 450 kt would
//! release it 1,000 ft above the target, aimed by the bomb equation
//! ([`tore_sim::combat::ballistics`]) to meet it where it will be. The run
//! prints the objectives and the debrief's tallies before and after, each
//! line starting `surface-objective:`.
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
use crate::{AppResult, replay, snapshot::RenderSnapshot, surface_dump};
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::{
    attitude::{Vector, unit},
    combat::{
        FallState,
        ballistics::{BombMotion, ReleaseState},
        ledger::ShotKind,
        live::Projectile,
    },
};
use tore_world::{
    ai_wings::{ENEMY_SIDE, FRIENDLY_SIDE},
    debrief,
    mission::{Defense, MissionSpec, Skill, WingSpec},
    seats::{SeatId, SeatInput},
    world::{Seating, TickOutput, World},
};

const SEAT: SeatId = SeatId(0);
const PLAYER: u32 = 0;
/// Ids for the bombs the run places, clear of the ownships' and the AI's.
const BOMB_ID_BASE: u32 = 0x00F0_0000;
const FEET_PER_NM: f64 = 6_076.;
const FPS_PER_KNOT: f64 = 1.687_8;
/// The pass each bomb is released from: a 45 degree dive at 450 kt, 1,000
/// ft above the target (fitted for this drive).
const BOMB_DIVE_DEG: f64 = 45.;
const BOMB_SPEED_KT: f64 = 450.;
const BOMB_HEIGHT_FT: f64 = 1_000.;

/// The release that puts a bomb's path on a target at `position` moving at
/// `velocity`: aimed where the target will be when the bomb gets there.
pub(crate) fn dive_release(position: Vector, velocity: Vector, heading: f64) -> ReleaseState {
    let dive = BOMB_DIVE_DEG.to_radians();
    let direction = [
        heading.sin() * dive.cos(),
        -dive.sin(),
        heading.cos() * dive.cos(),
    ];
    let speed = BOMB_SPEED_KT * FPS_PER_KNOT;
    let still = ReleaseState::onto(position, direction, speed, BOMB_HEIGHT_FT);
    let fall = BombMotion::new(still).fall_time(position[1]).unwrap_or(0.);
    let ahead = std::array::from_fn(|i| position[i] + [velocity[0], 0., velocity[2]][i] * fall);
    ReleaseState::onto(ahead, direction, speed, BOMB_HEIGHT_FT)
}

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
            other if other.starts_with("--") => return Err(format!("unknown {other}").into()),
            other => positional.push(other.to_owned()),
        }
    }
    let [theater, stem] = positional.as_slice() else {
        return Err("--surface-objective THEATER STEM [--surface-seed N] [--defenses AAA SAM] [--redfor] [--from NM] [--altitude FT] [--seconds S] [--kill-friendly] [--record PATH [--verify-render]] [--run-on] [--vulnerable]".into());
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
    let mut world = World::new(&spec, &resources, seating)?;
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
    let first = capture(&world)?;
    print("start", &first);
    let mut record = o
        .record
        .as_ref()
        .map(|path| {
            Record::start(
                &world,
                path,
                o.verify,
                "F/A-18D",
                "the surface objective run",
            )
        })
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
            let flight = &mut world.cockpits[0].flight;
            flight.position = [
                start[0] + forward[0] * speed * t,
                altitude,
                start[2] + forward[2] * speed * t,
            ];
            flight.velocity = [forward[0] * speed, 0., forward[2] * speed];
            flight.speed = speed;
            flight.yaw = heading;
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
                // Released from a dive pass, aimed ahead of a moving unit.
                let release = dive_release(row.position, row.velocity, heading);
                let from = release.position;
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
                    direction: unit(release.velocity),
                    speed_f8: (release.speed() * 256.) as i32,
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
/// is taken as live flight takes it and nothing it reads feeds back. The
/// air-to-ground driver (`--a2g-run --record`) records through it too.
pub(crate) struct Record {
    recorder: replay::recorder::Recorder,
    /// Every tick's picture, when the run checks the recording against them.
    pictures: Option<Vec<RenderSnapshot>>,
    devices: Vec<(u64, u64)>,
}

impl Record {
    pub(crate) fn start(
        world: &World,
        path: &std::path::Path,
        verify: bool,
        plane: &str,
        run: &str,
    ) -> AppResult<Self> {
        use replay::{convert, recorder};
        let combat = &world.combat;
        let flight = &world.cockpits[0].flight;
        let snapshot = combat.render_snapshot();
        let extra = vec![(
            "probe".to_owned(),
            format!("{run} on the full mission tick: no audio, music, HUD or rumble"),
        )];
        let header = recorder::header(
            tore_replay::MissionKind::Probe,
            &world.terrain,
            &convert::Presentation::of(snapshot),
            extra,
            std::time::SystemTime::now(),
        );
        let (player, others) = crate::recorded_humans(world, SEAT, plane);
        let roster = recorder::roster(
            snapshot,
            &player,
            &others,
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

    pub(crate) fn before(&mut self, world: &mut World) {
        self.recorder.start_tick(None, &mut world.combat);
    }

    pub(crate) fn after(&mut self, world: &mut World, out: &TickOutput) {
        let idle = tore_sim::flight::PilotInput::default();
        let cockpit = &world.cockpits[0];
        // The other human-flown planes (a buddy's seat) are recorded as the
        // game records them.
        let crews = crate::other_crews(&world.cockpits, cockpit.plane, &idle);
        self.recorder.begin(replay::recorder::Tick {
            snapshot: world.combat.render_snapshot(),
            combat: &world.combat,
            flight: &cockpit.flight,
            previous: &cockpit.previous_flight,
            pilot: &idle,
            others: &crews,
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
    pub(crate) fn finish(
        mut self,
        world: &World,
        report: &debrief::Report,
    ) -> AppResult<Vec<String>> {
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
