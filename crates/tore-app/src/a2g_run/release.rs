//! The `release` attack: a straight pass, level or in a dive, and the trigger
//! pressed when the bomb sight's mean impact reaches the target (bombs) or
//! one second after the station is selected (rockets and anything else).
//!
//! For every bomb it prints the release (its offset from the pack's first
//! bomb along the aircraft's right and up axes, its speed and its scatter),
//! the sight's solution, and where it burst against two predictions: its own
//! path from its actual release (`path-miss`, the motion check) and the mean
//! path without the scatter (`mean-miss`, inside `bound`, the farthest the
//! retail scatter can carry it). For other rounds it prints each tick that
//! fired and the rounds left.
use super::{FEET_PER_NM, FPS_PER_KNOT, Options, Scene};
use crate::AppResult;
use std::collections::BTreeMap;
use tore_sim::{
    attitude::{Basis, Vector, dot},
    combat::{
        ballistics::{self, BOMB_SCATTER_DEGREES, BombMotion, ReleaseState, predict_impact},
        blast::MarkKind,
        live::{EffectKind, Launcher},
    },
};

/// A bomb in flight and what it was predicted to do.
struct Falling {
    index: usize,
    last: Vector,
    path: Vector,
    mean: Vector,
    bound: f64,
}

pub(super) fn run(scene: &mut Scene, o: &Options) -> AppResult<()> {
    let aim = scene.target_row().ok_or("no target row")?.position;
    let aim = [aim[0], scene.ground(aim[0], aim[2]), aim[2]];
    let heading = o.heading_deg.to_radians();
    let pitch = -o.dive_deg.abs().to_radians();
    let speed = o.speed_kt * FPS_PER_KNOT;
    let forward = [
        heading.sin() * pitch.cos(),
        pitch.sin(),
        heading.cos() * pitch.cos(),
    ];
    let weapon = scene.station().weapon.clone();
    let bomb = ballistics::is_bomb(&weapon);
    // The pass runs through the point the release should happen at: for a
    // bomb, where its mean path meets the target; otherwise above it.
    let through = if bomb {
        ReleaseState::onto_body(
            aim,
            forward,
            speed,
            o.altitude,
            tore_sim::combat::bomb_flight::Body::of(&weapon),
        )
        .position
    } else {
        [aim[0], aim[1] + o.altitude, aim[2]]
    };
    let length = o.from_nm * FEET_PER_NM / pitch.cos();
    let start: Vector = std::array::from_fn(|i| through[i] - forward[i] * length);
    let along = |t: f64| -> Vector { std::array::from_fn(|i| start[i] + forward[i] * speed * t) };
    let launcher_at = |scene: &Scene, t: f64| Launcher {
        position: along(t),
        basis: Basis::new(heading, pitch, 0.),
        speed_fps: speed,
        velocity: forward.map(|v| v * speed),
        ..scene.launcher()
    };
    println!(
        "a2g-run: pass {} heading {:.1} dive {:.1} speed {:.1} kt release-height {:.0} from {:.2} nm",
        if bomb { "bomb" } else { "rocket" },
        o.heading_deg,
        o.dive_deg.abs(),
        o.speed_kt,
        o.altitude,
        o.from_nm
    );

    let rounds_start = scene.rounds();
    let press = o.hold.max(1. / 120.);
    let mut started: Option<f64> = None;
    let mut selected_at: Option<f64> = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut falling: BTreeMap<u32, Falling> = BTreeMap::new();
    let mut fired = 0usize;
    let mut released = 0usize;
    for tick in 0..(o.seconds * 120.) as u64 {
        let t = tick as f64 / 120.;
        scene.fly(along(t), heading, pitch, speed);
        scene.select();
        if scene.selected() {
            selected_at.get_or_insert(t);
        }
        if started.is_none() && scene.selected() {
            let due = if bomb {
                // The sight's mean impact, from where combat will see the
                // aircraft this tick, has reached the target.
                let release = ReleaseState::from_station(
                    &launcher_at(scene, t + 1. / 120.),
                    scene.station(),
                    0,
                );
                predict_impact(release, |x, z| scene.ground(x, z)).is_some_and(|impact| {
                    let gap: Vector = std::array::from_fn(|i| impact.position[i] - aim[i]);
                    gap[0] * forward[0] + gap[2] * forward[2] >= 0.
                })
            } else {
                selected_at.is_some_and(|at| t >= at + 1.)
            };
            if due {
                started = Some(t);
            }
        }
        let trigger = started.is_some_and(|at| {
            (0..o.repeat).any(|k| {
                let from = at + k as f64 * (press + o.gap);
                t >= from && t < from + press
            })
        });
        let before = scene.rounds();
        scene.step(trigger)?;
        let tick_now = scene.world.combat.state.tick();
        // What this tick released from the attack station.
        let fresh: Vec<(u32, Vector, Vector)> = scene
            .world
            .combat
            .state
            .projectiles
            .iter()
            .filter(|p| {
                p.owner == scene.owner && p.station == scene.station && !seen.contains(&p.id)
            })
            .map(|p| (p.id, p.position, p.previous))
            .collect();
        if !fresh.is_empty() {
            fired += fresh.len();
            println!(
                "a2g-run: fired t={:.3} projectiles {} rounds {}->{}",
                scene.seconds(),
                fresh.len(),
                before,
                scene.rounds()
            );
        }
        let launcher = scene.launcher();
        for (n, &(id, _, previous)) in fresh.iter().enumerate() {
            // Where every round started against the aircraft, feet right,
            // up and forward (B3): the forward figure includes the tick the
            // aircraft has flown since.
            let from: Vector = std::array::from_fn(|i| previous[i] - launcher.position[i]);
            // Where the store hung (PY2): the point the drawing hangs it
            // at, and how deep its pylon is.
            let station = scene.station();
            let units = tore_sim::combat::hardpoints::units(
                u32::from(scene.rounds()),
                u32::from(station.weapon.burst.actual_rounds_per_game),
            );
            let hung = station.release_point(units);
            println!(
                "a2g-run: origin n={n} id={id} right {:.2} up {:.2} forward {:.2} station-right {:.2} rounds-after {} hung {:.2} {:.2} {:.2} depth {}",
                dot(from, launcher.basis.right),
                dot(from, launcher.basis.up),
                dot(from, launcher.basis.forward),
                station.mount[0],
                scene.rounds(),
                hung[0],
                hung[1],
                hung[2],
                station
                    .release_depth(units)
                    .map_or("none".to_owned(), |d| format!("{d:.2}")),
            );
        }
        for (n, &(id, position, _)) in fresh.iter().enumerate() {
            seen.insert(id);
            let Some(motion) = scene.world.combat.state.bomb_motion(id) else {
                continue;
            };
            // The side of its station the bomb left from (B3): the
            // station's whole releases left after this one pick it.
            let station = scene.station();
            let mount = station.release_point(tore_sim::combat::hardpoints::units(
                u32::from(scene.rounds()),
                u32::from(station.weapon.burst.actual_rounds_per_game),
            ));
            let mean = ReleaseState::from_mount(&launcher, station, mount, n);
            let first = ReleaseState::from_mount(&launcher, station, mount, 0);
            if n == 0 {
                println!(
                    "a2g-run: release-mount right {:.2} up {:.2} forward {:.2} station-right {:.2}",
                    mount[0], mount[1], mount[2], station.mount[0]
                );
            }
            let actual = ReleaseState {
                body: motion.body,
                position: motion.origin,
                velocity: motion.velocity(0.),
            };
            let offset: Vector = std::array::from_fn(|i| motion.origin[i] - first.position[i]);
            println!(
                "a2g-run: bomb n={n} id={id} offset-right {:.2} offset-up {:.2} origin-error {:.4} speed {:.1} scatter-heading {:.3} scatter-pitch {:.3}",
                dot(offset, launcher.basis.right),
                offset[1],
                distance(motion.origin, mean.position),
                actual.speed(),
                (actual.heading() - mean.heading()).to_degrees(),
                (actual.pitch() - mean.pitch()).to_degrees(),
            );
            let ground = |x: f64, z: f64| scene.ground(x, z);
            let Some(predicted) = predict_impact(mean, ground) else {
                continue;
            };
            if n == 0 {
                let height = mean.position[1] - predicted.position[1];
                println!(
                    "a2g-run: solution t={:.3} height {:.2} speed {:.2} pitch {:.4} fall {:.4} throw {:.1} mean-impact {:.1} {:.1} {:.1} target-miss {:.1}",
                    scene.seconds(),
                    height,
                    mean.speed(),
                    mean.pitch().to_degrees(),
                    predicted.seconds,
                    horizontal(predicted.position, mean.position),
                    predicted.position[0],
                    predicted.position[1],
                    predicted.position[2],
                    horizontal(predicted.position, aim),
                );
            }
            let limit = BOMB_SCATTER_DEGREES.to_radians();
            let mut bound: f64 = 0.;
            for h in [-1., -0.5, 0., 0.5, 1.] {
                for p in [-1., -0.5, 0., 0.5, 1.] {
                    if let Some(corner) = predict_impact(mean.turned(h * limit, p * limit), ground)
                    {
                        bound = bound.max(horizontal(corner.position, predicted.position));
                    }
                }
            }
            falling.insert(
                id,
                Falling {
                    index: n,
                    last: position,
                    path: path_impact(&motion, &ground),
                    mean: predicted.position,
                    bound,
                },
            );
            released += 1;
        }
        // Bombs that burst this tick.
        let state = &scene.world.combat.state;
        let gone: Vec<u32> = falling
            .keys()
            .copied()
            .filter(|id| !state.projectiles.iter().any(|p| p.id == *id))
            .collect();
        for id in gone {
            let bomb = falling.remove(&id).expect("tracked");
            let candidates = state
                .marks
                .iter()
                .filter(|m| m.born == tick_now && matches!(m.kind, MarkKind::Crater(_)))
                .map(|m| m.position)
                .chain(
                    state
                        .effects
                        .iter()
                        .filter(|e| {
                            matches!(
                                e.kind,
                                EffectKind::Ground | EffectKind::Hit | EffectKind::Destroyed
                            )
                        })
                        .map(|e| e.position),
                );
            let burst = candidates
                .map(|at| (distance(at, bomb.last), at))
                .filter(|(d, _)| *d < 60.)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            match burst {
                Some((_, at)) => {
                    let mean_miss = horizontal(at, bomb.mean);
                    println!(
                        "a2g-run: impact n={} at {:.1} {:.1} {:.1} path-miss {:.2} mean-miss {:.1} bound {:.1} within {}",
                        bomb.index,
                        at[0],
                        at[1],
                        at[2],
                        horizontal(at, bomb.path),
                        mean_miss,
                        bound_with_slack(bomb.bound),
                        if mean_miss <= bound_with_slack(bomb.bound) {
                            "yes"
                        } else {
                            "no"
                        },
                    );
                }
                None => println!("a2g-run: impact n={} unseen", bomb.index),
            }
        }
        for (id, bomb) in falling.iter_mut() {
            if let Some(p) = state.projectiles.iter().find(|p| p.id == *id) {
                bomb.last = p.position;
            }
        }
        let presses_over =
            started.is_some_and(|at| t > at + o.repeat as f64 * (press + o.gap) + 0.5);
        if presses_over && falling.is_empty() {
            break;
        }
    }
    println!(
        "a2g-run: trigger presses {} hold {:.2} fired {fired} bombs {released} rounds-start {rounds_start} rounds-end {}",
        o.repeat,
        o.hold,
        scene.rounds()
    );
    if started.is_none() {
        println!("a2g-run: no release");
    }
    Ok(())
}

/// The scatter bound plus a little for the terrain under the burst, which
/// the predictor (ground under the release point) does not see.
fn bound_with_slack(bound: f64) -> f64 {
    bound + 50.
}

/// Where a bomb's own path meets the terrain: its closed-form path stepped at
/// the mission rate, the crossing bisected.
fn path_impact(motion: &BombMotion, ground: &impl Fn(f64, f64) -> f64) -> Vector {
    motion
        .impact(ground)
        .map_or(motion.origin, |impact| impact.position)
}

fn distance(a: Vector, b: Vector) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn horizontal(a: Vector, b: Vector) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}
