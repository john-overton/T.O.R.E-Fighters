//! `--surface-parked THEATER STEM [--surface-seed N] [--strike ORDINAL:WEAPON
//! ...] [--seconds S]`: a development aid and the battery's check of parked
//! aircraft (docs/spec/surface-defenses.md, "Parked aircraft"). It builds the
//! Quick Mission a ground target describes and prints every parked aircraft:
//! its type, class, hit points, gear word, size, where it stands and whether
//! it is on a carrier deck, and every fleet aircraft left out.
//!
//! `--strike` puts the player's own round on a parked aircraft (by its
//! template ordinal) one second in: `bomb` (a Mk 82 dropped on it from 300
//! ft), `maverick` (an AGM-65G flown into it from 3,000 ft out, after
//! checking its seeker sees the aircraft) or `gun` (a burst of 12 M61 rounds
//! through its left wing). The whole world then steps (the live game's tick,
//! no window, no audio) and the outcome is printed: hit points and damage by
//! section, kills with their debrief row, explosions, craters, fires and
//! fragments.
use crate::{AppResult, surface_dump};
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::combat::{
    FallState,
    blast::MarkKind,
    live::{EffectKind, Projectile},
    missiles::{Profile, TargetRole, seeker},
};
use tore_world::{
    mission::MissionSpec,
    seats::SeatInput,
    surface::SURFACE_UNIT_BASE,
    world::{Seating, TickOutput, World},
};

/// The scene object whose weapon contact volume holds `point`, if any: a
/// parked aircraft inside one is reached only through it. Runways meet
/// weapons at their pavement (`tore_world::combat::weapon_contact`).
fn sheltered(world: &World, point: [f64; 3]) -> Option<u32> {
    let below = [point[0], point[1] - 0.5, point[2]];
    let above = [point[0], point[1] + 0.5, point[2]];
    let scene = &world.terrain.airport_scene;
    scene
        .objects
        .iter()
        .find(|object| {
            let bounds = tore_world::combat::weapon_contact(object, scene).unwrap_or(object.bounds);
            bounds.segment_fraction(below, above).is_some()
                || bounds.segment_fraction(above, below).is_some()
        })
        .map(|object| object.id)
}

/// Ids for the rounds the drive places, clear of the ownships' and the AI's.
const STRIKE_ID_BASE: u32 = 0x00F0_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Strike {
    Bomb,
    Maverick,
    Gun,
}

pub fn run() -> AppResult<()> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let mut positional = Vec::new();
    let mut seed = 1u32;
    let mut seconds = 10.;
    let mut strikes: Vec<(u32, Strike)> = Vec::new();
    let mut clear_shelters = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut next = || {
            it.next()
                .ok_or_else(|| format!("{arg} needs a value"))
                .cloned()
        };
        match arg.as_str() {
            "--surface-seed" => seed = next()?.parse()?,
            "--seconds" => seconds = next()?.parse()?,
            "--clear-shelters" => clear_shelters = true,
            "--strike" => {
                let value = next()?;
                let (ordinal, weapon) = value
                    .split_once(':')
                    .ok_or("--strike ORDINAL:bomb|maverick|gun")?;
                let weapon = match weapon {
                    "bomb" => Strike::Bomb,
                    "maverick" => Strike::Maverick,
                    "gun" => Strike::Gun,
                    other => return Err(format!("unknown strike weapon {other}").into()),
                };
                strikes.push((ordinal.parse()?, weapon));
            }
            other if other.starts_with("--") => return Err(format!("unknown {other}").into()),
            other => positional.push(other.to_owned()),
        }
    }
    let [theater, stem] = positional.as_slice() else {
        return Err("--surface-parked THEATER STEM [--surface-seed N] [--strike ORDINAL:WEAPON ...] [--clear-shelters] [--seconds S]".into());
    };
    let mut resources = crate::reel::load_assets()?.theater_resources;
    surface_dump::with_retail_surface(&mut resources)?;
    let mut spec = MissionSpec::new(theater, AircraftId::F18);
    spec.ground_target = Some(stem.trim_start_matches('~').to_ascii_uppercase());
    spec.surface_seed = seed;
    // Experiment AP1: the redrawn airports when TORE_REDRAWN_AIRPORTS=1.
    spec.weather.redrawn_airports = crate::scenery::redrawn_airports()?;
    let mut world = World::new(&spec, &resources, Seating::SinglePlayer)?;
    let surface = world.terrain.surface.clone();
    if let Some(why) = &surface.unresolved {
        return Err(format!("the ground target stands nowhere: {why}").into());
    }
    for pose in &surface.parked_scene {
        let ground = world
            .terrain
            .height(pose.ground[0] as f32, pose.ground[2] as f32);
        println!(
            "surface-parked: aircraft {:#010x} {} class {:#06x} hp {} gear {} scale {:.4} ground {:.1} {:.1} {:.1} terrain {:.1} origin-up {:.2} heading {:.0} deck {} target {} side {}",
            pose.id.0,
            pose.resource,
            pose.class,
            pose.hit_points,
            pose.gear_word
                .map_or_else(|| "none".to_owned(), |w| format!("{w:#x}")),
            pose.scale,
            pose.ground[0],
            pose.ground[1],
            pose.ground[2],
            ground,
            pose.origin[1] - pose.ground[1],
            pose.heading.to_degrees(),
            pose.deck
                .map_or_else(|| "none".to_owned(), |id| format!("{:#010x}", id.0)),
            u8::from(pose.target),
            pose.side.0,
        );
    }
    if let Some(site) = &surface.template {
        for left in site.left_out.iter().filter(|l| l.resource.ends_with(".PT")) {
            println!(
                "surface-parked: left-out {} {} {}",
                left.ordinal, left.resource, left.why
            );
        }
    }
    let targets: Vec<u32> = surface.targets().map(|id| id.0).collect();
    println!(
        "surface-parked: parked {} placed {} targets {} parked-targets {} unreadable {}",
        surface.parked.len(),
        surface.parked_scene.len(),
        targets.len(),
        surface
            .parked_scene
            .iter()
            .filter(|p| targets.contains(&p.id.0))
            .count(),
        surface
            .unreadable
            .iter()
            .filter(|(r, _)| r.ends_with(".PT"))
            .count(),
    );
    for pose in &surface.parked_scene {
        let row = world
            .combat
            .state
            .targets
            .iter()
            .find(|t| t.id == pose.id.0);
        let Some(row) = row else {
            println!("surface-parked: row {:#010x} missing", pose.id.0);
            continue;
        };
        println!(
            "surface-parked: row {:#010x} role {} on-ground {} airborne {} parked {} sheltered {}",
            row.id,
            if row.role == TargetRole::Surface {
                "surface"
            } else {
                "aircraft"
            },
            u8::from(row.on_ground),
            u8::from(row.airborne),
            u8::from(world.combat.state.is_parked(row.id)),
            sheltered(&world, pose.origin)
                .map_or_else(|| "none".to_owned(), |id| format!("{id:#010x}")),
        );
    }

    // A debug aid: the scene objects whose contact volume holds a parked
    // aircraft (a building it stands in) stop every round before it; with
    // `--clear-shelters` they are knocked down first.
    if clear_shelters {
        let shelters: std::collections::BTreeSet<u32> = surface
            .parked_scene
            .iter()
            .filter_map(|pose| sheltered(&world, pose.origin))
            .collect();
        for id in shelters {
            if let Some(row) = world.combat.state.targets.iter_mut().find(|t| t.id == id) {
                row.hp = 0;
                println!("surface-parked: cleared shelter {id:#010x}");
            }
        }
    }
    let weapon = |name: &str| -> AppResult<Weapon> {
        let bytes = resources
            .get(name)
            .ok_or_else(|| format!("missing {name}"))?;
        Ok(Weapon::parse(name, bytes)?)
    };
    let owner = world.combat.own_id();
    let mut out = TickOutput::default();
    let mut struck = false;
    let mut next_id = STRIKE_ID_BASE;
    let mut blasts = std::collections::BTreeSet::new();
    for tick in 0..(seconds * 120.) as u64 {
        if !struck && tick >= 120 {
            struck = true;
            for (ordinal, kind) in &strikes {
                let id = SURFACE_UNIT_BASE + ordinal;
                let Some(row) = world.combat.state.targets.iter().find(|t| t.id == id) else {
                    println!("surface-parked: strike {ordinal}: no such parked aircraft");
                    continue;
                };
                let (center, basis) = (row.position, row.basis);
                let mut shots: Vec<(Weapon, [f64; 3], [f64; 3], f64)> = Vec::new();
                match kind {
                    Strike::Bomb => {
                        let from = [center[0], center[1] + 300., center[2]];
                        shots.push((weapon("MK82.JT")?, from, [0., -1., 0.], 500.));
                    }
                    Strike::Maverick => {
                        let w = weapon("AGM65G.JT")?;
                        let from = std::array::from_fn(|i| {
                            center[i] - basis.forward[i] * 1000. + [0., 2000., 0.][i]
                        });
                        let to: [f64; 3] = std::array::from_fn(|i| center[i] - from[i]);
                        let length = to.iter().map(|v| v * v).sum::<f64>().sqrt();
                        let direction = to.map(|v| v / length);
                        let profile = Profile::for_weapon(&w);
                        let observed = profile.as_ref().is_some_and(|profile| {
                            let view = seeker::View {
                                position: from,
                                basis: tore_sim::attitude::Basis::new(
                                    direction[0].atan2(direction[2]),
                                    direction[1].asin(),
                                    0.,
                                ),
                                cap: None,
                                obscured: &|_, _| false,
                            };
                            profile.accepts(row)
                                && seeker::observe(&w, *profile, &view, row).is_some()
                        });
                        println!(
                            "surface-parked: maverick {id:#010x} accepts {} seeker-lock {}",
                            u8::from(profile.is_some_and(|p| p.accepts(row))),
                            u8::from(observed)
                        );
                        shots.push((w, from, direction, 1000.));
                    }
                    Strike::Gun => {
                        // Diving at 45 degrees from behind onto the left wing.
                        let w = weapon("M61.JT")?;
                        let aim: [f64; 3] =
                            std::array::from_fn(|i| center[i] + basis.right[i] * -0.7 * row.radius);
                        let down: [f64; 3] = std::array::from_fn(|i| {
                            (basis.forward[i] - [0., 1., 0.][i]) / std::f64::consts::SQRT_2
                        });
                        for n in 0..12 {
                            let back = 300. + f64::from(n) * 20.;
                            let from = std::array::from_fn(|i| aim[i] - down[i] * back);
                            shots.push((w.clone(), from, down, 3300.));
                        }
                    }
                }
                for (w, from, direction, speed) in shots {
                    world.combat.state.projectiles.push(Projectile {
                        id: next_id,
                        owner,
                        weapon: Some(w),
                        guidance: None,
                        motion: None,
                        guidance_ticks: None,
                        age: 0,
                        incoming: None,
                        station: 0,
                        position: from,
                        previous: from,
                        direction,
                        speed_f8: (speed * 256.) as i32,
                        launched_t: 0,
                        target: None,
                        fall: FallState::default(),
                        gun_round: None,
                        tracer: false,
                    });
                    next_id += 1;
                }
                println!("surface-parked: t 1.0 strike {id:#010x} {kind:?}");
            }
        }
        let input = SeatInput {
            tick: world.tick(),
            ..SeatInput::default()
        };
        world.step(&[input], &mut out)?;
        for effect in &world.combat.state.effects {
            if effect.kind == EffectKind::Destroyed
                && let Some(blast) = effect.blast
            {
                let at = effect.position.map(|v| v.round() as i64);
                blasts.insert((at, blast));
            }
        }
    }
    let state = &world.combat.state;
    for pose in &surface.parked_scene {
        let Some(row) = state.targets.iter().find(|t| t.id == pose.id.0) else {
            continue;
        };
        if row.hp == row.initial_hp {
            continue;
        }
        let sections: Vec<String> = row
            .localized_damage
            .amounts
            .iter()
            .map(|a| a.to_string())
            .collect();
        let near = |kind: fn(&MarkKind) -> bool| {
            state
                .marks
                .iter()
                .filter(|m| kind(&m.kind))
                .filter(|m| {
                    (m.position[0] - pose.ground[0]).hypot(m.position[2] - pose.ground[2]) < 1.
                })
                .count()
        };
        println!(
            "surface-parked: outcome {:#010x} {} hp {}/{} sections {} structural {:?} craters {} fires {} fragment {}",
            row.id,
            pose.resource,
            row.hp,
            row.initial_hp,
            sections.join(","),
            row.localized_damage.structural_section,
            near(|k| matches!(k, MarkKind::Crater(_))),
            near(|k| matches!(k, MarkKind::Fire)),
            u8::from(row.fragment_released),
        );
    }
    for kill in state.ledger.kills() {
        println!(
            "surface-parked: kill {:#010x} by {} class {:#06x} row {:?} aircraft {}",
            kill.victim,
            kill.owner,
            kill.category,
            tore_world::debrief::kill_row(kill.category),
            u8::from(kill.aircraft)
        );
    }
    for (at, blast) in &blasts {
        println!(
            "surface-parked: explosion {blast} at {} {} {}",
            at[0], at[1], at[2]
        );
    }
    println!("surface-parked: done at {seconds:.0} s");
    Ok(())
}
