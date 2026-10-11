//! The `ccip` attack: a dive (or level) pass with an unguided bomb, released
//! through the trigger when the HUD's own CCIP pipper covers the target, as a
//! pilot does (docs/spec/air-to-ground.md, "CCIP, the dive pipper").
//!
//! The release decision reads combat's delivery solution, the one the HUD
//! draws, from the last tick: no prediction of the driver's own. It prints the
//! sight every half second (its arc steps, glyph, range and warnings), the
//! release, and where each bomb burst against the target. The pieces shared
//! with the `level` and `rocket` attacks live here too.
use super::{FEET_PER_NM, FPS_PER_KNOT, Options, Scene};
use crate::AppResult;
use std::collections::BTreeMap;
use tore_sim::{
    attitude::Vector,
    combat::{
        ballistics::{self, ReleaseState},
        blast::MarkKind,
        delivery::Delivery,
        live::EffectKind,
    },
};

/// A straight pass at the target.
pub(super) struct Pass {
    pub heading: f64,
    pub pitch: f64,
    pub speed: f64,
    pub forward: Vector,
    pub start: Vector,
    /// The target point on the ground.
    pub aim: Vector,
}

impl Pass {
    /// The pass through the point where a bomb's mean path (or, for anything
    /// else, the nose at `altitude` above the target) meets the target,
    /// starting `--from` nautical miles before it.
    pub fn new(scene: &Scene, o: &Options) -> AppResult<Self> {
        let row = scene.target_row().ok_or("no target row")?;
        let (now, velocity) = (row.position, row.velocity);
        let heading = o.heading_deg.to_radians();
        let pitch = -o.dive_deg.abs().to_radians();
        let speed = o.speed_kt * FPS_PER_KNOT;
        let forward = [
            heading.sin() * pitch.cos(),
            pitch.sin(),
            heading.cos() * pitch.cos(),
        ];
        let weapon = &scene.station().weapon;
        let length = o.from_nm * FEET_PER_NM / pitch.cos();
        // A moving target is led: the pass aims where it will be when the
        // round arrives, `from` of flying plus the fall later.
        let mut aim = now;
        let mut through = now;
        for _ in 0..3 {
            let ground = scene.ground(aim[0], aim[2]);
            let point = [aim[0], ground, aim[2]];
            let fall = if ballistics::is_bomb(weapon) {
                let release_speed =
                    tore_sim::combat::launch_speed(&weapon.movement, (speed * 256.) as i32)
                        .map_or(0., f64::from);
                let release = ReleaseState::onto(point, forward, release_speed, o.altitude);
                through = release.position;
                tore_sim::combat::ballistics::predict_impact(release, |_, _| ground)
                    .map_or(0., |impact| impact.seconds)
            } else {
                // Rockets: from `altitude` up, where the rocket's own fly-out
                // meets the target (the bore line, moved by the rocket's sink).
                let run = o.altitude / pitch.abs().tan().max(1e-3);
                let mut from = [
                    point[0] - forward[0] / pitch.cos() * run,
                    point[1] + o.altitude,
                    point[2] - forward[2] / pitch.cos() * run,
                ];
                let mut seconds = run / pitch.cos() / speed.max(1.);
                let probe = |at: Vector| tore_sim::combat::live::Launcher {
                    position: at,
                    basis: tore_sim::attitude::Basis::new(heading, pitch, 0.),
                    speed_fps: speed,
                    velocity: forward.map(|v| v * speed),
                    ..scene.launcher()
                };
                if let Some((_, impact)) = tore_sim::combat::delivery::rocket_impact(
                    weapon,
                    &probe(from),
                    scene.station(),
                    tore_sim::combat::delivery::Clock {
                        tick: 0,
                        service: 2,
                        remainder: 16,
                    },
                    &|_, _| ground,
                ) {
                    from[0] += point[0] - impact.position[0];
                    from[2] += point[2] - impact.position[2];
                    seconds = impact.seconds;
                }
                through = from;
                seconds
            };
            let lead = length / speed.max(1.) + fall;
            aim = std::array::from_fn(|i| now[i] + velocity[i] * lead);
        }
        let aim = [aim[0], scene.ground(aim[0], aim[2]), aim[2]];
        let start = std::array::from_fn(|i| through[i] - forward[i] * length);
        println!(
            "a2g-run: pass heading {:.1} dive {:.1} speed {:.1} kt release-height {:.0} from {:.2} nm",
            o.heading_deg,
            o.dive_deg.abs(),
            o.speed_kt,
            o.altitude,
            o.from_nm
        );
        Ok(Self {
            heading,
            pitch,
            speed,
            forward,
            start,
            aim,
        })
    }

    pub fn at(&self, t: f64) -> Vector {
        std::array::from_fn(|i| self.start[i] + self.forward[i] * self.speed * t)
    }

    pub fn fly(&self, scene: &mut Scene, t: f64) {
        scene.fly(self.at(t), self.heading, self.pitch, self.speed);
    }

    /// How far an impact `seconds` away at `point` lies past the target
    /// along the pass's ground track, the target led by its motion over those
    /// seconds: positive once the pipper is beyond where the target will be.
    pub fn past(&self, scene: &Scene, point: Vector, seconds: f64) -> f64 {
        let Some(row) = scene.target_row() else {
            return f64::NAN;
        };
        let led: Vector = std::array::from_fn(|i| row.position[i] + row.velocity[i] * seconds);
        let norm = self.forward[0].hypot(self.forward[2]).max(1e-9);
        ((point[0] - led[0]) * self.forward[0] + (point[2] - led[2]) * self.forward[2]) / norm
    }
}

/// The sight as a line: mode, arc, glyph, ranges and warnings.
pub(super) fn sight_line(scene: &Scene, d: &Delivery) -> String {
    let warnings = if d.warnings.is_empty() {
        "none".to_owned()
    } else {
        d.warnings
            .iter()
            .map(|w| w.label())
            .collect::<Vec<_>>()
            .join(",")
    };
    match &d.ccip {
        Some(c) => format!(
            "a2g-run: sight t={:.3} mode {} rocket {} arc {} glyph {:#04x} range {:.1} max {:.0} in-range {} slant {:.0} fall {:.3} impact {:.1} {:.1} {:.1} target-gap {:.1} warnings {warnings}",
            scene.seconds(),
            d.mode.label(),
            if d.rocket { "yes" } else { "no" },
            c.arc,
            c.glyph(),
            c.range_ft,
            c.maximum_ft,
            if c.in_range() { "yes" } else { "no" },
            c.slant_ft,
            c.seconds,
            c.impact[0],
            c.impact[1],
            c.impact[2],
            horizontal_gap(scene, c.impact),
        ),
        None => format!(
            "a2g-run: sight t={:.3} mode {} no-impact warnings {warnings}",
            scene.seconds(),
            d.mode.label()
        ),
    }
}

/// The horizontal distance from a point to the target.
fn horizontal_gap(scene: &Scene, point: Vector) -> f64 {
    scene.target_row().map_or(f64::NAN, |t| {
        (point[0] - t.position[0]).hypot(point[2] - t.position[2])
    })
}

/// The attack station's sight from the last tick.
pub(super) fn delivery(scene: &Scene) -> Option<Delivery> {
    let own = scene.world.combat.state.own();
    own.delivery
        .clone()
        .filter(|d| d.station == scene.station && own.armed)
}

/// Rounds in flight from the attack station and where each was last seen.
#[derive(Default)]
pub(super) struct Tracker {
    seen: std::collections::BTreeSet<u32>,
    falling: BTreeMap<u32, (usize, Vector, Option<Vector>)>,
    released: usize,
}

impl Tracker {
    /// Notes the rounds this tick released, each with the pipper it was
    /// fired on (`pipper`), and reports the ones that burst.
    pub fn update(&mut self, scene: &Scene, pipper: Option<Vector>) {
        let state = &scene.world.combat.state;
        let tick = state.tick();
        for p in state
            .projectiles
            .iter()
            .filter(|p| p.owner == scene.owner && p.station == scene.station)
        {
            if self.seen.insert(p.id) {
                self.falling
                    .insert(p.id, (self.released, p.position, pipper));
                self.released += 1;
            }
        }
        let gone: Vec<u32> = self
            .falling
            .keys()
            .copied()
            .filter(|id| !state.projectiles.iter().any(|p| p.id == *id))
            .collect();
        for id in gone {
            let (n, last, pipper) = self.falling.remove(&id).expect("tracked");
            let burst = state
                .marks
                .iter()
                .filter(|m| m.born == tick && matches!(m.kind, MarkKind::Crater(_)))
                .map(|m| (m.position, "ground"))
                .chain(state.effects.iter().filter_map(|e| {
                    let kind = match e.kind {
                        EffectKind::Ground => "ground",
                        EffectKind::Hit => "hit",
                        EffectKind::Destroyed => "destroyed",
                        _ => return None,
                    };
                    Some((e.position, kind))
                }))
                .map(|(at, kind)| (distance(at, last), at, kind))
                .filter(|(d, ..)| *d < 120.)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            match burst {
                Some((_, at, kind)) => println!(
                    "a2g-run: impact n={n} at {:.1} {:.1} {:.1} {kind} target-miss {:.1} pipper-miss {}",
                    at[0],
                    at[1],
                    at[2],
                    horizontal_gap(scene, at),
                    pipper.map_or("-".into(), |p| format!("{:.2}", distance(at, p))),
                ),
                None => println!("a2g-run: impact n={n} unseen"),
            }
        }
        for (id, entry) in self.falling.iter_mut() {
            if let Some(p) = state.projectiles.iter().find(|p| p.id == *id) {
                entry.1 = p.position;
            }
        }
    }

    pub fn flying(&self) -> bool {
        !self.falling.is_empty()
    }

    pub fn released(&self) -> usize {
        self.released
    }
}

fn distance(a: Vector, b: Vector) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

pub(super) fn run(scene: &mut Scene, o: &Options) -> AppResult<()> {
    let weapon = scene.station().weapon.clone();
    if !ballistics::is_bomb(&weapon) {
        return Err(format!("{} is not an unguided bomb", weapon.source).into());
    }
    let pass = Pass::new(scene, o)?;
    let mut tracker = Tracker::default();
    let mut pressed: Option<f64> = None;
    let mut next_line = 0.;
    for tick in 0..(o.seconds * 120.) as u64 {
        let t = tick as f64 / 120.;
        pass.fly(scene, t);
        scene.select();
        let sight = delivery(scene);
        if let Some(d) = &sight
            && t >= next_line
            && pressed.is_none()
        {
            println!("{}", sight_line(scene, d));
            next_line = t + 0.5;
        }
        // The pipper covers the target: press.
        let trigger = pressed.is_none()
            && sight
                .as_ref()
                .and_then(|d| d.ccip)
                .is_some_and(|c| pass.past(scene, c.impact, c.seconds) >= 0.);
        if trigger {
            let d = sight.as_ref().expect("a sight");
            println!(
                "{}",
                sight_line(scene, d).replace("a2g-run: sight", "a2g-run: release")
            );
            pressed = Some(t);
        }
        scene.step(trigger)?;
        tracker.update(scene, None);
        if pressed.is_some_and(|at| t > at + 0.5) && !tracker.flying() {
            break;
        }
    }
    println!(
        "a2g-run: released {} rounds-end {}",
        tracker.released(),
        scene.rounds()
    );
    if pressed.is_none() {
        println!("a2g-run: no release");
    }
    Ok(())
}
