//! `--flak-pass`: a development probe that flies a flight of aircraft on a
//! straight, scripted line over a team of heavy flak guns (or a lone gun) and
//! reports when the guns open fire, how their shells spread over the flight's
//! members, the damage each member takes, the blast knocks it feels and
//! whether the lead lives (docs/spec/surface-defenses.md, "Flak teams";
//! measurements: docs/baselines/flak-teams.md). No window opens; the whole
//! mission tick runs as in the game.
//!
//! ```text
//! tore-app --flak-pass THEATER [STEM] [--defenses AAA SAM] [--surface-seed N]
//!     [--list] [--team K | --size N] [--flight N] [--second N] [--seats] [--spacing FT] [--aircraft ID]
//!     [--altitude FT] [--speed KT] [--pass FT] [--from NM] [--heading DEG]
//!     [--seconds S] [--rng N] [--solo-team] [--quiet]
//! ```
//!
//! `--list` prints every flak team (KS-12 and KS-19 guns of one side chained
//! within a nautical mile) hostile to the flight and stops. `--team K` flies
//! over the Kth of them (default 0), `--size N` over the first of N guns. The flight is Friendly Wing 1 with
//! `--flight N` aircraft (default 2): the player and AI wingmen, or with
//! `--seats` every member flown from a seat as players are online;
//! `--second N` adds Friendly Wing 2's N aircraft flying on in the same
//! formation (one group of up to eight). Each
//! member flies the lead's line at its formation place (`--spacing`, 1,500 ft:
//! member k sits k/2 spacings back and the same out to alternate sides) and
//! is put back on it every tick, so a gun's choice cannot be dodged. The line
//! passes `--pass` feet east of the team's middle (`--heading` turns it).
//! `--solo-team` lays down every armed unit's weapons but the team's, and
//! the SAM batteries'.
//! `--rng` fixes the surface's random stream.
use crate::AppResult;
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::aircraft::AircraftId;
use tore_sim::{
    attitude::Vector,
    combat::live::{self, EffectKind, Event, Jolt},
};
use tore_world::{
    ai_wings::FRIENDLY_SIDE,
    mission::{Defense, MissionSpec},
    seats::{SeatId, SeatInput, Slot},
    surface::{
        UnitId,
        fire::{Arms, Kind, Trace},
    },
    world::{Seating, TickOutput, World},
};

const FEET_PER_NM: f64 = 6_076.;
const FPS_PER_KNOT: f64 = 1.687_8;
/// The probe's own reading of a flak team: the rule the game uses
/// (`tore_world::surface::fire::flak_teams`), repeated so the probe picks the
/// same guns on a build without teams.
const TEAM_RADIUS_FT: f64 = 6_076.;

struct Options {
    theater: String,
    stem: Option<String>,
    defenses: (usize, usize),
    seed: u32,
    list: bool,
    team: usize,
    size: Option<usize>,
    flight: u8,
    second: u8,
    seats: bool,
    spacing: f64,
    aircraft: AircraftId,
    altitude: f64,
    speed_kt: f64,
    pass: f64,
    from_nm: f64,
    heading_deg: f64,
    seconds: f64,
    rng: Option<u64>,
    solo_team: bool,
    quiet: bool,
}

fn options() -> AppResult<Options> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let mut o = Options {
        theater: String::new(),
        stem: None,
        defenses: (3, 0),
        seed: 1,
        list: false,
        team: 0,
        size: None,
        flight: 2,
        second: 0,
        seats: false,
        spacing: 1_500.,
        aircraft: AircraftId::F18,
        altitude: 8_000.,
        speed_kt: 450.,
        pass: 0.,
        from_nm: 12.,
        heading_deg: 0.,
        seconds: 150.,
        rng: None,
        solo_team: false,
        quiet: false,
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
            "--defenses" => o.defenses = (next()?.parse()?, next()?.parse()?),
            "--surface-seed" => o.seed = next()?.parse()?,
            "--list" => o.list = true,
            "--team" => o.team = next()?.parse()?,
            "--size" => o.size = Some(next()?.parse()?),
            "--flight" => o.flight = next()?.parse()?,
            "--second" => o.second = next()?.parse()?,
            "--seats" => o.seats = true,
            "--spacing" => o.spacing = next()?.parse()?,
            "--aircraft" => o.aircraft = AircraftId::parse(&next()?)?,
            "--altitude" => o.altitude = next()?.parse()?,
            "--speed" => o.speed_kt = next()?.parse()?,
            "--pass" => o.pass = next()?.parse()?,
            "--from" => o.from_nm = next()?.parse()?,
            "--heading" => o.heading_deg = next()?.parse()?,
            "--seconds" => o.seconds = next()?.parse()?,
            "--rng" => o.rng = Some(next()?.parse()?),
            "--solo-team" => o.solo_team = true,
            "--quiet" => o.quiet = true,
            other if other.starts_with("--") => return Err(format!("unknown {other}").into()),
            other => positional.push(other.to_owned()),
        }
    }
    match positional.as_slice() {
        [theater] => o.theater = theater.to_ascii_uppercase(),
        [theater, stem] => {
            o.theater = theater.to_ascii_uppercase();
            o.stem = Some(stem.trim_start_matches('~').to_ascii_uppercase());
        }
        _ => return Err("--flak-pass THEATER [STEM] [options]; see flak_pass.rs".into()),
    }
    if o.second > 4 {
        return Err("--second is 0 to 4".into());
    }
    if o.flight == 0 || o.flight > 4 {
        return Err("--flight is 1 to 4".into());
    }
    Ok(o)
}

fn spec(o: &Options) -> AppResult<MissionSpec> {
    use tore_formats::quick_template::tables;
    let mut spec = MissionSpec::new(&o.theater, o.aircraft);
    spec.wings[0].count = usize::from(o.flight);
    spec.wings[1].count = usize::from(o.second);
    let index = tables::THEATERS
        .iter()
        .position(|t| t.eq_ignore_ascii_case(&o.theater))
        .ok_or_else(|| format!("{}: not a Quick Mission theater", o.theater))?;
    spec.ground_target = o.stem.clone();
    spec.aaa = Defense::from_level(o.defenses.0).ok_or("defense levels are 0 to 3")?;
    spec.sam = Defense::from_level(o.defenses.1).ok_or("defense levels are 0 to 3")?;
    spec.surface_seed = o.seed;
    spec.enemy_nationality = tables::ENEMY_NATIONALITY[index] as u8;
    spec.validate()?;
    Ok(spec)
}

fn is_flak_gun(arms: &Arms) -> bool {
    !arms.ship
        && arms.battery.is_none()
        && arms
            .weapons
            .iter()
            .any(|w| matches!(w.kind, Kind::Gun { flak: true, .. }) && !w.battery)
}

/// The hostile flak teams, in order of their first gun.
fn teams(world: &World) -> Vec<Vec<&Arms>> {
    let guns: Vec<&Arms> = world
        .terrain
        .surface
        .arsenal
        .units
        .iter()
        .filter(|a| is_flak_gun(a) && a.side != FRIENDLY_SIDE && a.side != live::NO_SIDE)
        .collect();
    let mut set: Vec<usize> = (0..guns.len()).collect();
    // Chains: relabel until nothing changes (a handful of guns).
    loop {
        let mut changed = false;
        for i in 0..guns.len() {
            for j in 0..guns.len() {
                let (a, b) = (guns[i], guns[j]);
                let near = a.side == b.side
                    && (a.position[0] - b.position[0]).hypot(a.position[2] - b.position[2])
                        <= TEAM_RADIUS_FT;
                if near && set[j] > set[i] {
                    set[j] = set[i];
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut out: BTreeMap<usize, Vec<&Arms>> = BTreeMap::new();
    for (gun, s) in guns.into_iter().zip(set) {
        out.entry(s).or_default().push(gun);
    }
    out.into_values().collect()
}

fn resource(world: &World, id: UnitId) -> String {
    world
        .terrain
        .surface
        .unit(id)
        .map_or("?".into(), |u| u.resource.clone())
}

/// The share of the fitted knock a jolt event carries.
fn knock_share(jolt: &Jolt) -> f64 {
    jolt.share
}

#[derive(Default)]
struct Member {
    first_hp: i32,
    shells: u32,
    first_shell: Option<f64>,
    opening: u32,
    destroyed: Option<f64>,
    jolts: u32,
    knock: f64,
}

fn hp(world: &World, id: u32) -> i32 {
    world.combat.state.ownship(id).map_or_else(
        || {
            world
                .combat
                .state
                .targets
                .iter()
                .find(|t| t.id == id)
                .map_or(0, |t| t.hp)
        },
        |own| own.hp,
    )
}

pub fn run() -> AppResult<()> {
    let o = options()?;
    let resources = crate::surface_trace::resources()?;
    let spec = spec(&o)?;
    let seating = if o.seats {
        Seating::Open
    } else {
        Seating::SinglePlayer
    };
    let mut world = World::build(
        &spec,
        &resources,
        seating,
        &mut tore_world::world::Hooks {
            ground_variation: Some(tore_world::surface::layout::Variation {
                jitter: true,
                relocate: true,
                front: false,
            }),
            ..Default::default()
        },
    )?
    .world;
    if let Some(why) = &world.terrain.surface.unresolved {
        return Err(why.clone().into());
    }
    let all = teams(&world);
    for (k, team) in all.iter().enumerate() {
        let xs: Vec<String> = team
            .iter()
            .map(|a| {
                format!(
                    "{:#x} {} at {:.0} {:.0}",
                    a.unit.0,
                    resource(&world, a.unit),
                    a.position[0],
                    a.position[2]
                )
            })
            .collect();
        println!(
            "flak-pass: team {k} guns {} [{}]",
            team.len(),
            xs.join("; ")
        );
    }
    if o.list {
        return Ok(());
    }
    let mut o = o;
    if let Some(size) = o.size {
        o.team = all
            .iter()
            .position(|team| team.len() == size)
            .ok_or_else(|| format!("no flak team of {size} guns"))?;
    }
    if let Some(team) = all.get(o.team) {
        for arms in team {
            for w in arms
                .weapons
                .iter()
                .filter(|w| matches!(w.kind, Kind::Gun { flak: true, .. }))
            {
                println!(
                    "flak-pass: gun {:#x} {} detection {:.0}..{:.0} ft alt {:.0}..{:.0} fire {:.0}..{:.0} ft alt {:.0}..{:.0} search-limit {:?} skill {}",
                    arms.unit.0,
                    w.record.source,
                    w.detection.min_range,
                    w.detection.max_range,
                    w.detection.min_altitude.max(-1e9),
                    w.detection.max_altitude.min(1e9),
                    w.launch.min_range,
                    w.launch.max_range,
                    w.launch.min_altitude.max(-1e9),
                    w.launch.max_altitude.min(1e9),
                    arms.search_limit,
                    arms.skill,
                );
            }
        }
    }
    let chosen: Vec<UnitId> = all
        .get(o.team)
        .ok_or_else(|| format!("no flak team {}", o.team))?
        .iter()
        .map(|a| a.unit)
        .collect();
    let middle: Vector = {
        let arsenal = &world.terrain.surface.arsenal;
        let mut sum = [0.; 3];
        for id in &chosen {
            let a = arsenal.arms(*id).expect("listed");
            for (s, p) in sum.iter_mut().zip(a.position) {
                *s += p;
            }
        }
        sum.map(|v| v / chosen.len() as f64)
    };
    if o.solo_team {
        let keep: BTreeSet<UnitId> = chosen.iter().copied().collect();
        let arsenal = &mut world.terrain.surface.arsenal;
        for other in &mut arsenal.units {
            if !keep.contains(&other.unit) {
                other.weapons.clear();
            }
        }
        // SAM batteries fire from their radar's controller.
        arsenal.batteries.clear();
    }
    if let Some(rng) = o.rng {
        world.combat.surface.rng = rng;
    }
    // The flight: Friendly Wing 1 by member, then Friendly Wing 2's
    // (`--second`) flying on in the same formation.
    let lead_wing = Slot::FRIENDLY_LEAD.wing;
    let mut members: Vec<(u8, u8, u32)> = world
        .roster
        .planes()
        .iter()
        .filter(|p| {
            p.slot.wing.side == lead_wing.side
                && (p.slot.wing.index == 0 || (p.slot.wing.index == 1 && o.second > 0))
        })
        .map(|p| (p.slot.wing.index, p.slot.member, p.id.0))
        .collect();
    members.sort();
    let ids: Vec<u32> = members.iter().map(|(_, _, id)| *id).collect();
    if o.seats {
        for (seat, (_, _, id)) in members.iter().enumerate() {
            world.take_plane(SeatId(seat as u8), tore_world::seats::PlaneId(*id))?;
        }
    }
    println!(
        "flak-pass: team {} middle {:.0} {:.0} {:.0} flight {:?} {} altitude {:.0} speed {:.0} kt pass {:.0} ft heading {:.0}",
        o.team,
        middle[0],
        middle[1],
        middle[2],
        ids,
        if o.seats { "seats" } else { "player and AI" },
        o.altitude,
        o.speed_kt,
        o.pass,
        o.heading_deg
    );
    let heading = o.heading_deg.to_radians();
    let forward = [heading.sin(), 0., heading.cos()];
    let right = [heading.cos(), 0., -heading.sin()];
    let speed = o.speed_kt * FPS_PER_KNOT;
    let altitude = middle[1] + o.altitude;
    let start: Vector = std::array::from_fn(|i| {
        middle[i] - forward[i] * o.from_nm * FEET_PER_NM + right[i] * o.pass
    });
    // Formation places: member k is k/2 spacings back and out, alternate sides.
    let place = |k: usize| -> (f64, f64) {
        let rank = k.div_ceil(2) as f64;
        let side = if k % 2 == 1 { 1. } else { -1. };
        (rank * o.spacing * side, -rank * o.spacing * 0.5)
    };
    let mut stats: BTreeMap<u32, Member> = ids
        .iter()
        .map(|id| {
            (
                *id,
                Member {
                    first_hp: hp(&world, *id),
                    ..Member::default()
                },
            )
        })
        .collect();
    let team_set: BTreeSet<UnitId> = chosen.iter().copied().collect();
    let mut out = TickOutput::default();
    let mut first_shell: Option<f64> = None;
    let mut flak_seen: BTreeSet<[u64; 3]> = BTreeSet::new();
    let mut bursts = 0;
    let ticks = (o.seconds * 120.) as u64;
    for tick in 0..ticks {
        let t = tick as f64 / 120.;
        for (k, id) in ids.iter().enumerate() {
            if stats[id].destroyed.is_some() {
                continue;
            }
            let (side, back) = place(k);
            let position = [
                start[0] + forward[0] * (speed * t + back) + right[0] * side,
                altitude,
                start[2] + forward[2] * (speed * t + back) + right[2] * side,
            ];
            let flight = match world.cockpits.iter_mut().find(|c| c.plane.0 == *id) {
                Some(cockpit) => Some(&mut cockpit.flight),
                None => world
                    .ai_wings
                    .as_mut()
                    .and_then(|wings| wings.actor_flight_mut(*id)),
            };
            if let Some(flight) = flight {
                flight.position = position;
                flight.velocity = [forward[0] * speed, 0., forward[2] * speed];
                flight.speed = speed;
                flight.yaw = heading;
                flight.pitch = 0.;
                flight.bank = 0.;
                flight.vertical_speed = 0.;
                flight.roll_rate = 0.;
                flight.pitch_rate = 0.;
            }
        }
        let inputs: Vec<SeatInput> = world
            .cockpits
            .iter()
            .enumerate()
            .map(|(seat, _)| SeatInput {
                seat: SeatId(seat as u8),
                tick: world.tick(),
                ..Default::default()
            })
            .collect();
        world.step(&inputs, &mut out)?;
        for trace in &world.combat.surface.trace {
            match trace {
                Trace::Shot {
                    unit,
                    target,
                    rounds,
                    opening,
                    flak: true,
                    ..
                } if team_set.contains(unit) && *rounds > 0 => {
                    first_shell.get_or_insert(t);
                    if let Some(m) = stats.get_mut(target) {
                        m.shells += rounds;
                        m.first_shell.get_or_insert(t);
                        if *opening {
                            m.opening += rounds;
                        }
                    }
                    if !o.quiet {
                        println!(
                            "flak-pass: t={t:.2} shot {:#x} {} target {target}{}",
                            unit.0,
                            resource(&world, *unit),
                            if *opening { " opening" } else { "" }
                        );
                    }
                }
                Trace::FlakTeam { .. } => {
                    println!(
                        "flak-pass: t={t:.2} {}",
                        crate::surface_trace::describe(trace)
                    );
                }
                _ => {}
            }
        }
        for effect in &world.combat.state.effects {
            if effect.kind == EffectKind::Flak {
                let key = effect.position.map(f64::to_bits);
                if flak_seen.insert(key) {
                    bursts += 1;
                }
            }
        }
        for event in &out.events {
            if !o.quiet
                && let Event::Destroyed(id)
                | Event::OwnshipDestroyed { aircraft: id }
                | Event::OwnshipGroundImpact { aircraft: id } = event
                && stats.contains_key(id)
            {
                println!("flak-pass: t={t:.2} event {event:?}");
            }
            if let Event::Jolt(jolt) = event
                && let Some(m) = stats.get_mut(&jolt.target)
            {
                m.jolts += 1;
                m.knock += jolt.strength.clamp(0.5, 2.) * knock_share(jolt);
            }
        }
        for id in &ids {
            let now = hp(&world, *id);
            let m = stats.get_mut(id).expect("member");
            if now <= 0 && m.destroyed.is_none() {
                m.destroyed = Some(t);
                println!("flak-pass: t={t:.2} member {id} destroyed");
            }
        }
    }
    let mut spread = Vec::new();
    for (k, id) in ids.iter().enumerate() {
        let m = &stats[id];
        let now = hp(&world, *id).max(0);
        spread.push(m.shells);
        println!(
            "flak-pass: member {id}{} shells {} opening {} first-shell {} hp {} -> {now} lost {} destroyed {} jolts {} knock {:.2}",
            if k == 0 { " (lead)" } else { "" },
            m.shells,
            m.opening,
            m.first_shell.map_or("-".into(), |v| format!("{v:.2}")),
            m.first_hp,
            m.first_hp - now,
            m.destroyed.map_or("-".into(), |v| format!("{v:.2}")),
            m.jolts,
            m.knock,
        );
    }
    let lost: i32 = ids
        .iter()
        .map(|id| stats[id].first_hp - hp(&world, *id).max(0))
        .sum();
    println!(
        "flak-pass: summary first-shell {} shells {:?} bursts {bursts} damage {lost} lead-survived {}",
        first_shell.map_or("-".into(), |v| format!("{v:.2}")),
        spread,
        stats[&ids[0]].destroyed.is_none()
    );
    Ok(())
}
