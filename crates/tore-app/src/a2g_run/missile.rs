//! The air-to-ground missile attacks (slice W2, docs/spec/air-to-ground.md,
//! "Missiles and the HARM boresight cue", "Weapon classes" 8 to 11):
//! `maverick`, `harm`, `antiship` and `sarh`.
//!
//! The aircraft flies a straight, level pass at the target, `--altitude` feet
//! above the ground under it, from `--from` nautical miles. Everything goes
//! through the player's own controls: the scope channel (`--designate ir` or
//! `harm`, the M key), the A-G command (`ag` and `visual`, Ctrl+R) half a second in, the
//! designation key two seconds in (T until the target is the selection, or
//! Enter for `visual`; `none` designates nothing, so an infrared or
//! anti-radiation missile is in BORE), the weapon keys, and the trigger once
//! the weapon reads READY. `--break-at S` turns the A-G page off S seconds
//! after the launch (the SARH check). The missile is followed to its end.
//!
//! Lines, each starting `a2g-run:`: `missile` (the weapon and its profile),
//! `designated`, `harm-cue` (the HUD cue's emitters and candidate), `ready`,
//! `launch`, `flight` once a second, `shutdown` and `radar` for the target's
//! radar, `break`, `end` (where the missile went and the target's hit
//! points), or `no launch` with the last readiness.
use super::{FEET_PER_NM, FPS_PER_KNOT, Options, SEAT, Scene};
use crate::{AppResult, aircraft, hud, weapon_hud};
use std::collections::BTreeMap;
use tore_formats::weapons::Weapon;
use tore_sim::{
    attitude::Vector,
    combat::{
        live::{Command, Readiness},
        missiles::{LaunchMode, Profile, TargetRole, seeker},
    },
    sensors::{Channel, Controls, profile::RANGE_LADDER_NMI},
};
use tore_world::{
    seats::{SeatCommand, SeatInput},
    surface::{UnitId, fire::Trace},
};

/// A reviewed air-to-ground missile: a profiled record with the surface
/// target role.
pub(super) fn air_to_ground(w: &Weapon) -> bool {
    Profile::for_weapon(w).is_some_and(|p| p.role == TargetRole::Surface)
}

/// Seconds the trigger is held for the launch.
const PRESS: f64 = 0.1;

pub(super) fn run(
    scene: &mut Scene,
    o: &Options,
    kind: &str,
    resources: &BTreeMap<String, Vec<u8>>,
) -> AppResult<()> {
    if let Some(skill) = o.skill {
        set_skill(scene, skill, resources);
    }
    let weapon = scene.station().weapon.clone();
    let profile = Profile::for_weapon(&weapon)
        .ok_or_else(|| format!("{} has no missile profile", weapon.source))?;
    let how = o
        .designate
        .clone()
        .unwrap_or_else(|| if kind == "harm" { "none" } else { "ag" }.into());
    let mut controls = Controls {
        channel: match how.as_str() {
            "ir" => Channel::Infrared,
            "harm" => Channel::Harm,
            _ => Channel::Radar,
        },
        ..Controls::default()
    };
    // The scope range that holds the target: T cycles contacts inside it.
    controls.range_index = RANGE_LADDER_NMI
        .iter()
        .position(|range| *range >= o.from_nm + 1.)
        .unwrap_or(RANGE_LADDER_NMI.len() - 1);
    println!(
        "a2g-run: missile {} kind {kind} guidance {:?} role {:?} designate {how} channel {:?} range {} nm",
        weapon.source,
        profile.guidance,
        profile.role,
        controls.channel,
        controls.range_nmi()
    );
    let row = scene.target_row().ok_or("no target row")?;
    let aim = row.position;
    let ground = scene.ground(aim[0], aim[2]);
    let heading = o.heading_deg.to_radians();
    let speed = o.speed_kt * FPS_PER_KNOT;
    let forward = [heading.sin(), 0., heading.cos()];
    let start: Vector = [
        aim[0] - forward[0] * o.from_nm * FEET_PER_NM,
        ground + o.altitude,
        aim[2] - forward[2] * o.from_nm * FEET_PER_NM,
    ];
    terrain_line(scene, "setup", start, aim);
    let along = |t: f64| -> Vector { std::array::from_fn(|i| start[i] + forward[i] * speed * t) };

    let mut air_ground_sent = false;
    let mut designations = 0;
    let mut designated_at: Option<f64> = None;
    let mut pressed: Option<f64> = None;
    let mut missile: Option<u32> = None;
    let mut last: Option<(Vector, String)> = None;
    let mut broke = false;
    let mut ended_at: Option<f64> = None;
    let mut readiness = Readiness::Safe;
    let mut cue_printed = false;
    let mut traced = 0;
    for tick in 0..(o.seconds * 120.) as u64 {
        let t = tick as f64 / 120.;
        scene.fly(along(t), heading, 0., speed);
        scene.select();
        let own = scene.world.combat.state.own();
        // Enter picks among the page's visible ground contacts, so `visual`
        // turns the A-G page on too.
        if matches!(how.as_str(), "ag" | "visual") && !air_ground_sent && t >= 0.5 {
            scene
                .commands
                .push(SeatCommand::Combat(Command::ToggleAirGround));
            air_ground_sent = true;
        }
        let selected = own.sensors.selected();
        if !matches!(how.as_str(), "none")
            && designated_at.is_none()
            && t >= 2.
            && scene.commands.is_empty()
        {
            // Without `--unit` the first contact the key designates is the
            // target; with it, the key is pressed until that one is.
            if o.unit.is_none()
                && let Some(id) = selected
            {
                scene.target = id;
            }
            if selected == Some(scene.target) {
                designated_at = Some(t);
                println!(
                    "a2g-run: designated t={t:.3} {:#010x} presses {designations}",
                    scene.target
                );
            } else if designations < 24 && tick.is_multiple_of(30) {
                designations += 1;
                let command = if how == "visual" {
                    Command::DesignateVisual
                } else {
                    Command::Designate
                };
                scene.commands.push(SeatCommand::Combat(command));
            }
        }
        // The HUD's HARM cue, once it shows something.
        if !cue_printed && !own.harm_cue().is_empty() {
            print_cue(scene, t);
            cue_printed = true;
        }
        let launcher = scene.launcher();
        readiness = scene
            .world
            .combat
            .state
            .view(scene.owner)
            .map_or(Readiness::Safe, |view| {
                view.at(launcher)
                    .with_terrain(&|x, z| scene.ground(x, z))
                    .readiness()
            });
        // In BORE with `--unit` the trigger waits until the cue's candidate
        // is that unit.
        let wanted = if how == "none" {
            o.unit.is_none()
                || scene
                    .world
                    .combat
                    .state
                    .own()
                    .bore_observation
                    .map(|b| b.id)
                    == Some(scene.target)
        } else {
            designated_at.is_some()
        };
        // A render waits for the blinking diamond's lit half.
        let lit = o.render.is_none()
            || scene
                .world
                .combat
                .state
                .own()
                .sensors
                .tick()
                .is_multiple_of(60);
        if pressed.is_none() && wanted && lit && scene.selected() && readiness == Readiness::Ready {
            let own = scene.world.combat.state.own();
            println!(
                "a2g-run: ready t={t:.3} mode {} candidate {} designated {} seeker {:?} target {}",
                mode(own.launch_mode),
                id(own.bore_observation.map(|o| o.id)),
                id(own.designated()),
                own.mounted.status,
                id(own.mounted.target),
            );
            print_cue(scene, t);
            if own.launch_mode == LaunchMode::Boresight {
                print_bore(scene, &weapon, profile, t);
            }
            if let Some(path) = &o.render {
                render(scene, resources, path)?;
            }
            terrain_line(
                scene,
                "release",
                launcher.position,
                scene.target_row().map_or(aim, |r| r.position),
            );
            pressed = Some(t);
        }
        let trigger = pressed.is_some_and(|at| t < at + PRESS);
        scene.step_with(trigger, controls)?;
        let state = &scene.world.combat.state;
        if missile.is_none()
            && let Some(p) = state
                .projectiles
                .iter()
                .find(|p| p.owner == scene.owner && p.station == scene.station)
        {
            missile = Some(p.id);
            // In BORE without `--unit` the target is what the missile left for.
            if o.unit.is_none()
                && how == "none"
                && let Some(id) = p.target
            {
                scene.target = id;
            }
            let flight = p.guidance.as_ref();
            println!(
                "a2g-run: launch t={:.3} id {} target {} mode {} seeker {} acquired {} unguided {} rounds {}",
                scene.seconds(),
                p.id,
                id(p.target),
                flight.map_or("-", |f| mode(f.mode)),
                flight.map_or_else(|| "none".into(), |f| format!("{:?}", f.seeker.status)),
                flight.is_some_and(|f| f.seeker.acquired),
                flight.is_some_and(|f| f.unguided),
                scene.rounds()
            );
        }
        // The target's radar and its HARM shutdown rolls.
        let trace = &scene.world.combat.surface.trace;
        for entry in trace.iter().skip(traced) {
            match entry {
                Trace::Shutdown {
                    unit,
                    missile: m,
                    rolled,
                } if unit.0 == scene.target => println!(
                    "a2g-run: shutdown t={:.3} unit {:#010x} missile {m} shuts {}",
                    scene.seconds(),
                    unit.0,
                    u8::from(*rolled)
                ),
                Trace::Radar { unit, on } if unit.0 == scene.target => println!(
                    "a2g-run: radar t={:.3} unit {:#010x} {}",
                    scene.seconds(),
                    unit.0,
                    if *on { "on" } else { "off" }
                ),
                _ => {}
            }
        }
        traced = trace.len();
        if let Some(id) = missile {
            let state = &scene.world.combat.state;
            match state.projectiles.iter().find(|p| p.id == id) {
                Some(p) => {
                    let status = p.guidance.as_ref().map_or_else(
                        || "none".into(),
                        |f| {
                            format!(
                                "{:?} target {} unguided {}",
                                f.seeker.status,
                                self::id(p.target),
                                u8::from(f.unguided)
                            )
                        },
                    );
                    if tick.is_multiple_of(120) {
                        let target = scene.target_row().map_or(aim, |r| r.position);
                        println!(
                            "a2g-run: flight t={:.3} id {id} range {:.0} altitude {:.0} {status}",
                            scene.seconds(),
                            distance(p.position, target),
                            p.position[1]
                        );
                    }
                    last = Some((p.position, status));
                }
                None if ended_at.is_none() => {
                    let target = scene.target_row().map_or(aim, |r| r.position);
                    let (at, status) = last.clone().unwrap_or((target, "unseen".into()));
                    let hp = scene.target_row().map_or((0, 0), |r| (r.hp, r.initial_hp));
                    println!(
                        "a2g-run: end t={:.3} id {id} at {:.0} {:.0} {:.0} miss {:.0} last {status} target-hp {}/{}",
                        scene.seconds(),
                        at[0],
                        at[1],
                        at[2],
                        distance(at, target),
                        hp.0,
                        hp.1
                    );
                    ended_at = Some(t);
                }
                None => {}
            }
        }
        if let (Some(at), Some(after)) = (pressed, o.break_at)
            && !broke
            && missile.is_some()
            && t >= at + after
        {
            scene
                .commands
                .push(SeatCommand::Combat(Command::ToggleAirGround));
            broke = true;
            println!("a2g-run: break t={t:.3} air-ground off");
        }
        if ended_at.is_some_and(|at| t > at + 1.) {
            break;
        }
    }
    if pressed.is_none() {
        println!("a2g-run: no launch readiness {}", readiness.label());
    }
    Ok(())
}

impl Scene {
    /// One mission tick with the trigger, the scope controls and the queued
    /// commands.
    fn step_with(&mut self, trigger: bool, controls: Controls) -> AppResult<()> {
        let input = SeatInput {
            seat: SEAT,
            tick: self.world.tick(),
            trigger,
            sensors: controls,
            commands: std::mem::take(&mut self.commands),
            ..SeatInput::default()
        };
        self.recorded(|world, out| world.step(std::slice::from_ref(&input), out))
    }
}

/// The HUD cue the readout builds for the HARM: its emitters, the candidate
/// and whether it is cued.
fn print_cue(scene: &Scene, t: f64) {
    let Some(readout) = scene.world.cockpit_readout(SEAT, scene.launcher()) else {
        return;
    };
    let Some(cue) = &readout.estimates.harm else {
        return;
    };
    let list: Vec<String> = cue
        .emitters
        .iter()
        .map(|e| {
            format!(
                "{:#010x}:{}{}",
                e.id,
                if e.painting { "p" } else { "-" },
                if e.emitting { "e" } else { "d" }
            )
        })
        .collect();
    let painting: Vec<String> = scene
        .world
        .combat
        .state
        .own()
        .painted_by()
        .iter()
        .map(|id| format!("{id:#010x}"))
        .collect();
    println!(
        "a2g-run: harm-cue t={t:.3} painted-by {} emitters {} candidate {} cued {} list {}",
        if painting.is_empty() {
            "none".into()
        } else {
            painting.join(",")
        },
        cue.emitters.len(),
        id(cue.candidate),
        u8::from(cue.cued),
        if list.is_empty() {
            "none".into()
        } else {
            list.join(",")
        }
    );
}

/// What the seeker sees in its bore circle: each return's identity, class,
/// infrared signature and strength, strongest first, so a check can see the
/// BORE candidate is the hottest return near the bore.
fn print_bore(scene: &Scene, weapon: &Weapon, profile: Profile, t: f64) {
    let launcher = scene.launcher();
    let ground = |x: f64, z: f64| scene.ground(x, z);
    let obscured = |from: Vector, to: Vector| {
        !tore_sim::combat::missiles::launch_line_clear(from, to, &ground)
    };
    let view = seeker::View {
        position: launcher.position,
        basis: launcher.basis,
        cap: Some(profile.search_cap()),
        obscured: &obscured,
    };
    let mut seen: Vec<(f64, String)> = scene
        .world
        .combat
        .state
        .targets
        .iter()
        .filter(|t| t.body_present())
        .filter_map(|t| {
            let o = seeker::observe(weapon, profile, &view, t)?;
            let score = o.quality * seeker::centre_weight(o.off_axis, profile.search_cap());
            Some((
                score,
                format!(
                    "{:#010x}:{:#06x}:{:.0}:{:.3}",
                    t.id, t.category, t.signature.infrared, score
                ),
            ))
        })
        .collect();
    seen.sort_by(|a, b| b.0.total_cmp(&a.0));
    let list: Vec<String> = seen.into_iter().map(|(_, text)| text).collect();
    println!(
        "a2g-run: bore-cone t={t:.3} returns {} list {}",
        list.len(),
        if list.is_empty() {
            "none".into()
        } else {
            list.join(",")
        }
    );
}

/// The target and every unit of its type within a mile (its battery mates)
/// at experience `skill`, the arsenal reloaded, as `--surface-trace --skill`
/// does: a deterministic HARM shutdown roll.
fn set_skill(scene: &mut Scene, skill: i32, resources: &BTreeMap<String, Vec<u8>>) {
    let world = &mut scene.world;
    let surface = &mut world.terrain.surface;
    let Some((here, resource)) = surface
        .unit(UnitId(scene.target))
        .map(|u| (u.position, u.resource.clone()))
    else {
        return;
    };
    for u in &mut surface.units {
        let d = f64::from(u.position[0] - here[0]).hypot(f64::from(u.position[2] - here[2]));
        if u.id.0 == scene.target || (u.resource == resource && d <= FEET_PER_NM) {
            u.skill = skill;
        }
    }
    let terrain = &world.terrain;
    let arsenal = tore_world::surface::fire::Arsenal::load(&terrain.surface, resources, &|x, z| {
        f64::from(terrain.height(x as f32, z as f32))
    });
    world.terrain.surface.arsenal = arsenal;
    world.combat.surface = world.terrain.surface.fresh_state();
    println!("a2g-run: skill {skill} for {:#010x}", scene.target);
}

/// The HUD as the cockpit draws it now, over a plain sky and ground, four
/// times the size.
fn render(
    scene: &Scene,
    resources: &BTreeMap<String, Vec<u8>>,
    path: &std::path::Path,
) -> AppResult<()> {
    let world = &scene.world;
    let config = world.combat.state.own().configuration();
    let airframe = aircraft::Airframe::load(resources, config.aircraft)?;
    let color = airframe.daylight_palette()[usize::from(airframe.hud.primary_color)];
    let picture = world.combat.render_snapshot();
    let frame = world
        .flight_frame(SEAT, None, picture, &[])
        .ok_or("no flight frame")?;
    let flight = frame.presented().clone();
    let (width, height) = (crate::menu::WIDTH, crate::menu::HEIGHT);
    let mut pixels = vec![0u8; width * height * 4];
    hud::draw(
        &mut pixels,
        &flight,
        &airframe.hud_font,
        scene.ground(flight.position[0], flight.position[2]),
        None,
        true,
        weapon_hud::active(&frame),
        color,
        1.,
        None,
        None,
        (flight.bank, 1.),
    );
    weapon_hud::draw(
        &mut pixels,
        &frame,
        &airframe.hud_font,
        color,
        1.,
        false,
        true,
        None,
        false,
    );
    let horizon = hud::project(flight.pitch, flight.bank, 0., 0., 1.)
        .map_or(240., |(_, y)| y)
        .clamp(0., height as f64);
    const SCALE: usize = 2;
    let mut rgba = Vec::with_capacity(width * height * SCALE * SCALE * 4);
    for y in 0..height * SCALE {
        for x in 0..width * SCALE {
            let at = ((y / SCALE) * width + x / SCALE) * 4;
            let back: [u8; 3] = if ((y / SCALE) as f64) < horizon {
                [52, 88, 140]
            } else {
                [64, 84, 52]
            };
            let pixel = &pixels[at..at + 4];
            if pixel[3] == 0 {
                rgba.extend_from_slice(&[back[0], back[1], back[2], 255]);
            } else {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        path,
        crate::replay::png::encode_rgba((width * SCALE) as u32, (height * SCALE) as u32, &rgba)?,
    )?;
    println!("a2g-run: rendered {}", path.display());
    Ok(())
}

fn mode(mode: LaunchMode) -> &'static str {
    match mode {
        LaunchMode::Cued => "CUED",
        LaunchMode::Boresight => "BORE",
    }
}

fn id(id: Option<u32>) -> String {
    id.map_or_else(|| "none".into(), |id| format!("{id:#010x}"))
}

fn distance(a: Vector, b: Vector) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

/// Sample the measured route for repeatable ridge cases, independent of the
/// weapon's range cue. Positive mask means terrain rises above the sight line.
fn terrain_line(scene: &Scene, label: &str, from: Vector, to: Vector) {
    let distance = (to[0] - from[0]).hypot(to[2] - from[2]);
    let samples = (distance / 128.).ceil().max(2.) as u32;
    let mut peak = f64::NEG_INFINITY;
    let mut mask = f64::NEG_INFINITY;
    let mut ridge_at = 0.;
    for i in 1..samples {
        let share = f64::from(i) / f64::from(samples);
        let p: Vector = std::array::from_fn(|axis| from[axis] + (to[axis] - from[axis]) * share);
        let height = scene.ground(p[0], p[2]);
        if height > peak {
            peak = height;
            ridge_at = share * distance;
        }
        mask = mask.max(height - p[1]);
    }
    println!(
        "a2g-run: terrain {label} from {:.0} {:.0} {:.0} target {:.0} {:.0} {:.0} peak {:.0} at {:.0} mask {:.0}",
        from[0], from[1], from[2], to[0], to[1], to[2], peak, ridge_at, mask
    );
}
