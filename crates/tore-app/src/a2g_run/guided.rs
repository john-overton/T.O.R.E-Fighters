//! The guided attacks of `--a2g-run` (slice W1): `laser`, `jdam` and `at2`
//! (docs/spec/air-to-ground.md, "Weapon classes" 4, 5, 7 and 12).
//!
//! Each flies a straight pass at the target, `--altitude FT` above it at
//! `--speed KT` along `--heading DEG`, starting `--from NM` short of it. The
//! target is designated through the player's own keys: the A-G radar page
//! and T (nearest contact first) on an aircraft with a radar, else after 8 s
//! the Easy targeting cheat's Ctrl+X (an aircraft with no ground page), and
//! the run then attacks whatever that designated. The trigger is pressed
//! once the HUD's guided sight says IN RNG and the aim has come a quarter of
//! the way into the reach (the release range scale's cue). Every weapon is
//! followed to its end and reported against the target.
//!
//! A guided bomb in LVL holds the trigger for the level T's bar (slice FX-H).
//! By default the run flies the press (it puts the sight in CCIP with Ctrl+M
//! when the selection chose LVL); `--release bar` flies the T instead: LVL,
//! the trigger held from the designation, and the release left to combat.
//!
//! Options of this module only (the shared parser skips them):
//!
//! - `--laser-off S`: Ctrl+L turns the laser off S seconds after the release
//!   (the laser bomb then falls blind).
//! - `--buddy ID`: aircraft ID flies 2 nm behind and 3,000 ft right of the
//!   attacker on a second seat, designates with Ctrl+X (Easy targeting) and
//!   lases by hand (Ctrl+L): the attacker drops on the buddy's spot.
//! - `--buddy-ai` (with `--buddy`): the buddy is the AI's instead (slice
//!   ST-L): it starts where the seat's buddy would, is ordered to lase the
//!   attacker's designated target until it dies (`AiWings::order_lase`, the
//!   call of the wing order "Lase my target") and flies the offset turn
//!   itself. Prints `buddy-ai-ordered` and, when its laser first lases,
//!   `buddy-lasing` with the callsign the target page would name it by.
//! - `--turn-away S`: the attacker turns 90 degrees right S seconds after the
//!   release (an AT-2's launcher loses its sight of the target).
//! - `--condition clear|cloudy|foggy|night`: the mission's weather.
//! - `--release cue|bar`: `cue` (default) presses the trigger a quarter of
//!   the way into the reach; `bar` holds it in LVL and lets the T release.
//!
//! Lines start `a2g-run:`; the battery's `a2g_guided.py` reads them.
use super::{FEET_PER_NM, FPS_PER_KNOT, Options, SEAT, Scene, fit_store};
use crate::AppResult;
use std::collections::BTreeMap;
use tore_formats::aircraft::AircraftId;
use tore_sim::{
    attitude::Vector,
    combat::{
        delivery::DeliveryMode,
        guided::{self, GuidedSight},
        laser::LaserState,
        live::{Command, GuidedFlight},
    },
};
use tore_world::{
    mission::{Condition, Defense, MissionSpec, WingSpec},
    seats::{SeatCommand, SeatId, SeatInput},
    world::{Seating, TickOutput, World},
};

/// The attack kinds of this module.
pub(super) const KINDS: [&str; 3] = ["laser", "jdam", "at2"];
/// The options this module reads, each with one value.
pub(super) const OPTIONS: [&str; 5] = [
    "--laser-off",
    "--buddy",
    "--turn-away",
    "--condition",
    "--release",
];
/// The buddy's seat.
const BUDDY_SEAT: SeatId = SeatId(1);

#[derive(Default)]
pub(super) struct Extra {
    laser_off: Option<f64>,
    buddy: Option<AircraftId>,
    /// `--buddy-ai`: the AI flies the buddy.
    buddy_ai: bool,
    turn_away: Option<f64>,
    condition: Option<Condition>,
    /// `--release bar`: hold the trigger in LVL.
    bar: bool,
}

fn extra() -> AppResult<Extra> {
    let args: Vec<String> = std::env::args().collect();
    let mut e = Extra {
        buddy_ai: args.iter().any(|a| a == "--buddy-ai"),
        ..Extra::default()
    };
    for pair in args.windows(2) {
        let value = &pair[1];
        match pair[0].as_str() {
            "--laser-off" => e.laser_off = Some(value.parse()?),
            "--buddy" => e.buddy = Some(AircraftId::parse(value)?),
            "--turn-away" => e.turn_away = Some(value.parse()?),
            "--release" => {
                e.bar = match value.as_str() {
                    "bar" => true,
                    "cue" => false,
                    other => {
                        return Err(format!("unknown --release {other}; use cue or bar").into());
                    }
                }
            }
            "--condition" => {
                e.condition = Some(match value.to_ascii_lowercase().as_str() {
                    "clear" => Condition::Clear,
                    "cloudy" => Condition::Cloudy,
                    "foggy" => Condition::Foggy,
                    "night" => Condition::Night,
                    other => return Err(format!("unknown --condition {other}").into()),
                })
            }
            _ => {}
        }
    }
    Ok(e)
}

/// The two seats of a buddy run: the buddy's plane.
pub(super) struct Buddy {
    plane: u32,
    commands: Vec<SeatCommand>,
}

/// Builds the mission, seats the attacker (and the buddy), fits the store and
/// picks the target, as `Scene::new` does for the other kinds.
pub(super) fn scene(
    o: &Options,
    e: &Extra,
    resources: &BTreeMap<String, Vec<u8>>,
) -> AppResult<(Scene, Option<Buddy>)> {
    let mut spec = MissionSpec::new(&o.theater, o.aircraft);
    spec.ground_target = Some(o.stem.clone());
    spec.aaa = Defense::from_level(o.defenses.0).ok_or("defense levels are 0 to 3")?;
    spec.sam = Defense::from_level(o.defenses.1).ok_or("defense levels are 0 to 3")?;
    spec.surface_seed = o.seed;
    spec.cheats.damage = tore_sim::cheats::Damage::Invulnerable;
    if let Some(condition) = e.condition {
        spec.condition = condition;
    }
    if let Some(buddy) = e.buddy {
        spec.wings[1] = WingSpec {
            aircraft: buddy,
            count: 1,
            ..spec.wings[0]
        };
        spec.cheats.easy_targeting = true;
    }
    spec.validate()?;
    let seating = if e.buddy.is_some() {
        Seating::Open
    } else {
        Seating::SinglePlayer
    };
    let mut world = World::new(&spec, resources, seating)?;
    if let Some(why) = &world.terrain.surface.unresolved {
        return Err(format!("the ground target stands nowhere: {why}").into());
    }
    let lead = |world: &World, wing: usize| {
        world
            .roster
            .planes()
            .iter()
            .find(|p| MissionSpec::wing_index(p.slot.wing) == wing && p.slot.member == 0)
            .map(|p| p.id)
    };
    let buddy = if e.buddy.is_some() {
        let attacker = lead(&world, 0).ok_or("no attacker plane")?;
        let buddy = lead(&world, 1).ok_or("no buddy plane")?;
        world.take_plane(SEAT, attacker)?;
        world.take_plane(BUDDY_SEAT, buddy)?;
        Some(Buddy {
            plane: buddy.0,
            commands: Vec::new(),
        })
    } else {
        None
    };
    let owner = world
        .roster
        .seat(SEAT)
        .and_then(|seat| seat.plane)
        .ok_or("the seat flies no plane")?
        .0;
    let targets =
        tore_world::ai_wings::outcome::ground_targets(&world.terrain.surface, &world.combat.state);
    let target = *targets
        .get(o.target)
        .ok_or_else(|| format!("{} has {} ground targets", o.stem, targets.len()))?;
    let station = fit_store(&mut world, owner, o, resources)?;
    let scene = Scene {
        world,
        out: TickOutput::default(),
        commands: Vec::new(),
        station,
        target,
        owner,
        record: None,
    };
    let scene = scene.recording(o)?;
    let row = scene.target_row().ok_or("the target has no row")?;
    let own = scene
        .world
        .combat
        .state
        .ownship(owner)
        .ok_or("no ownship")?;
    let s = &own.configuration().stations[station];
    println!(
        "a2g-run: setup aircraft {:?} station {station} {} count {} rounds {} target {target:#010x} at {:.0} {:.0} {:.0} hp {}",
        o.aircraft,
        s.weapon.source,
        s.count,
        own.rounds(station),
        row.position[0],
        row.position[1],
        row.position[2],
        row.hp
    );
    Ok((scene, buddy))
}

/// One weapon followed to its end.
struct Followed {
    last: Vector,
    flight: GuidedFlight,
    released: f64,
    reported: u64,
}

/// A pass: where the aircraft is at `t` seconds, and its heading.
#[derive(Clone, Copy)]
struct Pass {
    start: Vector,
    heading: f64,
    speed: f64,
    /// The run's time the pass starts at.
    t0: f64,
}
impl Pass {
    fn forward(&self) -> Vector {
        [self.heading.sin(), 0., self.heading.cos()]
    }
    fn at(&self, t: f64) -> Vector {
        let f = self.forward();
        std::array::from_fn(|i| self.start[i] + f[i] * self.speed * (t - self.t0))
    }
}

impl Scene {
    fn cockpit_mut(&mut self, plane: u32) -> Option<&mut tore_world::world::Cockpit> {
        self.world.cockpits.iter_mut().find(|c| c.plane.0 == plane)
    }

    /// Puts `plane`'s cockpit at `position`, level along `heading`.
    fn place(&mut self, plane: u32, position: Vector, heading: f64, speed: f64) {
        let Some(cockpit) = self.cockpit_mut(plane) else {
            return;
        };
        let flight = &mut cockpit.flight;
        let forward = [heading.sin(), 0., heading.cos()];
        flight.position = position;
        flight.velocity = forward.map(|v| v * speed);
        flight.speed = speed;
        flight.yaw = heading;
        flight.pitch = 0.;
        flight.bank = 0.;
        flight.vertical_speed = 0.;
        flight.roll_rate = 0.;
        flight.pitch_rate = 0.;
    }

    /// One tick with the attacker's trigger and the queued commands, and
    /// the buddy's commands on its seat.
    fn step_seats(&mut self, trigger: bool, buddy: Option<&mut Buddy>) -> AppResult<()> {
        let mut inputs = vec![SeatInput {
            seat: SEAT,
            tick: self.world.tick(),
            trigger,
            commands: std::mem::take(&mut self.commands),
            ..SeatInput::default()
        }];
        if let Some(buddy) = buddy {
            inputs.push(SeatInput {
                seat: BUDDY_SEAT,
                tick: self.world.tick(),
                commands: std::mem::take(&mut buddy.commands),
                ..SeatInput::default()
            });
        }
        self.recorded(|world, out| world.step(&inputs, out))
    }

    fn sight(&self) -> Option<GuidedSight> {
        self.world.combat.state.ownship(self.owner)?.guided_sight
    }

    fn designated(&self, plane: u32) -> Option<u32> {
        self.world.combat.state.ownship(plane)?.designated()
    }
}

/// Runs a guided attack kind from start to end, prints `a2g-run: done`.
pub(super) fn run(o: &Options, resources: &BTreeMap<String, Vec<u8>>) -> AppResult<()> {
    let e = extra()?;
    let (mut scene, mut buddy) = scene(o, &e, resources)?;
    let mut ai_buddy: Option<u32> = None;
    let w = scene.station().weapon.clone();
    let kind = guided::kind(&w).ok_or_else(|| {
        format!(
            "{} is not a guided air-to-ground weapon (laser, JDAM or AT-2)",
            w.source
        )
    })?;
    println!(
        "a2g-run: guided-record {} kind {kind:?} glide {} zone1 {}..{} angle {} turn {} final {} retard {} life {:.0} s",
        w.source,
        guided::is_glide_bomb(&w),
        w.seeker.zones[1].minimum_range,
        w.seeker.zones[1].maximum_range,
        w.seeker.zones[1].heading,
        w.movement.unpowered_turn_rate,
        w.movement.final_speed,
        w.movement.launch_retard,
        f64::from(w.movement.remove_t) / 4.,
    );
    let row = scene.target_row().ok_or("no target row")?.position;
    let heading = o.heading_deg.to_radians();
    let speed = o.speed_kt * FPS_PER_KNOT;
    let ground = scene.ground(row[0], row[2]);
    let mut pass = Pass {
        start: {
            let back = o.from_nm * FEET_PER_NM;
            [
                row[0] - heading.sin() * back,
                ground + o.altitude,
                row[2] - heading.cos() * back,
            ]
        },
        heading,
        speed,
        t0: 0.,
    };
    // The buddy: 2 nm behind, 3,000 ft right and 2,000 ft higher, at the
    // same speed.
    let buddy_pass = Pass {
        start: {
            let f = pass.forward();
            let right = [f[2], 0., -f[0]];
            std::array::from_fn(|i| {
                pass.start[i] - f[i] * 2. * FEET_PER_NM + right[i] * 3_000. + [0., 2_000., 0.][i]
            })
        },
        ..pass
    };
    // An AI buddy starts where the seat's buddy would and is then flown by the
    // AI: its seat gives the plane back.
    if e.buddy_ai
        && let Some(b) = buddy.take()
    {
        scene.place(b.plane, buddy_pass.at(0.), buddy_pass.heading, speed);
        scene.world.give_back_plane(BUDDY_SEAT)?;
        ai_buddy = Some(b.plane);
    }
    let mut turned: Option<f64> = None;
    let mut laser_off_done = false;
    let mut ai_buddy_ordered = false;
    let mut ai_buddy_lased = false;
    let mut released: Option<f64> = None;
    let mut agpage = false;
    let mut followed: BTreeMap<u32, Followed> = BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut reported_sight = false;
    let mut designated_at: Option<f64> = None;
    let mut lased_at: Option<f64> = None;
    let mut last_reach_line = -10.;
    for tick in 0..(o.seconds * 120.) as u64 {
        let t = tick as f64 / 120.;
        // The attacker's pose: straight, or turned away after the release.
        let (position, heading_now) = match (turned, released, e.turn_away) {
            (Some(at), _, _) => {
                let away = pass.heading + std::f64::consts::FRAC_PI_2;
                let corner = pass.at(at);
                let f = [away.sin(), 0., away.cos()];
                (
                    std::array::from_fn(|i| corner[i] + f[i] * speed * (t - at)),
                    away,
                )
            }
            (None, Some(r), Some(after)) if t >= r + after => {
                turned = Some(t);
                println!("a2g-run: turn-away t={:.3}", scene.seconds());
                (pass.at(t), pass.heading + std::f64::consts::FRAC_PI_2)
            }
            _ => (pass.at(t), pass.heading),
        };
        let owner = scene.owner;
        scene.place(owner, position, heading_now, speed);
        if let Some(b) = &buddy {
            let plane = b.plane;
            scene.place(plane, buddy_pass.at(t), buddy_pass.heading, speed);
        }
        scene.select();
        // Designate: the buddy with Ctrl+X, the attacker through its page.
        if let Some(b) = buddy.as_mut() {
            if scene.designated(b.plane).is_none() && tick % 60 == 0 {
                b.commands.push(SeatCommand::Combat(Command::NearestGround));
            }
            let lasing = scene
                .world
                .combat
                .state
                .ownship(b.plane)
                .map(|own| own.laser);
            if let (Some(id), Some(laser)) = (scene.designated(b.plane), lasing) {
                if designated_at.is_none() {
                    designated_at = Some(t);
                    scene.target = id;
                    println!(
                        "a2g-run: buddy-designated t={:.3} target {id:#010x}",
                        scene.seconds()
                    );
                }
                if !laser.wanted() && tick % 60 == 30 {
                    b.commands.push(SeatCommand::Combat(Command::ToggleLaser));
                }
                if laser.state == LaserState::Lasing && lased_at.is_none() {
                    lased_at = Some(t);
                    println!(
                        "a2g-run: buddy-lasing t={:.3} range {:.0}",
                        scene.seconds(),
                        laser.range_ft
                    );
                }
            }
        } else if released.is_none() && scene.designated(owner).is_none() {
            if !agpage {
                scene
                    .commands
                    .push(SeatCommand::Combat(Command::ToggleAirGround));
                agpage = true;
            } else if t > 8. {
                // No ground page answers: the Easy targeting cheat's Ctrl+X.
                scene.world.combat.state.cheats.easy_targeting = true;
                scene
                    .commands
                    .push(SeatCommand::Combat(Command::NearestGround));
            } else if tick % 60 == 30 {
                // T: the nearest contact on the A-G page.
                scene.commands.push(SeatCommand::Combat(Command::Designate));
            }
        } else if released.is_none()
            && let Some(id) = scene.designated(owner)
            && designated_at.is_none()
        {
            designated_at = Some(t);
            scene.target = id;
            println!(
                "a2g-run: designated t={:.3} target {id:#010x} {}",
                scene.seconds(),
                scene.world.combat.ground_name(id).unwrap_or("unnamed")
            );
            // Flying the stem: the bar run turns onto the designated object
            // (the run otherwise flies at the row it was given).
            if e.bar
                && let Some(at) = scene.target_row().map(|r| r.position)
            {
                let from = pass.at(t);
                pass = Pass {
                    start: from,
                    heading: (at[0] - from[0]).atan2(at[2] - from[2]),
                    speed,
                    t0: t,
                };
            }
        }
        // The AI buddy is ordered onto the attacker's target once it is
        // designated, as the wing order would send it, and lases it.
        if let Some(plane) = ai_buddy {
            if !ai_buddy_ordered
                && designated_at.is_some()
                && let Some(wings) = scene.world.ai_wings.as_mut()
            {
                ai_buddy_ordered = true;
                let ok = wings.order_lase(
                    plane,
                    scene.target,
                    tore_sim::ai::strike::LaseUntil::TargetDies,
                );
                println!(
                    "a2g-run: buddy-ai-ordered t={:.3} plane={plane} ok={ok} target {:#010x}",
                    scene.seconds(),
                    scene.target
                );
            }
            let laser = scene
                .world
                .combat
                .state
                .strike_cockpit(plane)
                .map(|c| c.laser);
            if let Some(laser) = laser
                && laser.state == LaserState::Lasing
                && !ai_buddy_lased
            {
                ai_buddy_lased = true;
                let name = scene
                    .world
                    .ai_wings
                    .as_ref()
                    .and_then(|w| w.callsign(plane))
                    .unwrap_or_default();
                println!(
                    "a2g-run: buddy-lasing t={:.3} range {:.0} callsign \"{name}\"",
                    scene.seconds(),
                    laser.range_ft
                );
            }
        }
        let sight = scene.sight();
        if let Some(sight) = sight
            && released.is_none()
            && t - last_reach_line >= 2.
        {
            last_reach_line = t;
            print_sight("sight", &scene, &sight);
        }
        // A guided bomb's selection picks LVL on a level pass: the cue run
        // wants the plain press (CCIP), the bar run wants the T.
        let mode = scene
            .world
            .combat
            .state
            .ownship(owner)
            .map(|own| own.delivery_mode);
        let bar_run = e.bar && guided::is_glide_bomb(&w);
        if guided::is_glide_bomb(&w)
            && released.is_none()
            && scene.selected()
            && scene.commands.is_empty()
            && mode.is_some_and(|m| (m == DeliveryMode::Level) != e.bar)
        {
            scene
                .commands
                .push(SeatCommand::Combat(Command::ToggleSeekerMode));
        }
        // The release: IN RNG, a quarter of the way into the reach; for the
        // bar run the held trigger from the designation on, in LVL.
        let due = released.is_none()
            && scene.selected()
            && designated_at.is_some_and(|at| t >= at + 0.5)
            && (!bar_run
                || (mode == Some(DeliveryMode::Level) && sight.is_some_and(|s| s.level.is_some())))
            && (bar_run
                || sight.is_some_and(|s| {
                    s.in_reach
                        && match (s.reach, s.ground_range_ft) {
                            (Some(reach), Some(range)) => {
                                !guided::is_glide_bomb(&w) || reach.fraction(range) <= 0.75
                            }
                            _ => true,
                        }
                }));
        if due && !reported_sight && !bar_run {
            reported_sight = true;
            if let Some(sight) = sight {
                print_sight("release-sight", &scene, &sight);
            }
        }
        // The laser off by hand after the release.
        if let (Some(r), Some(after)) = (released, e.laser_off)
            && !laser_off_done
            && t >= r + after
        {
            laser_off_done = true;
            scene
                .commands
                .push(SeatCommand::Combat(Command::ToggleLaser));
            println!("a2g-run: laser-off t={:.3}", scene.seconds());
        }
        scene.step_seats(due, buddy.as_mut())?;
        let state = &scene.world.combat.state;
        if bar_run && !reported_sight && state.guided_flights().any(|(_, f)| f.owner == owner) {
            reported_sight = true;
            if let Some(sight) = sight {
                print_sight("release-sight", &scene, &sight);
            }
        }
        for (id, flight) in state.guided_flights() {
            if flight.owner != owner || seen.contains(&id) {
                continue;
            }
            seen.insert(id);
            released.get_or_insert(t);
            let p = state.projectiles.iter().find(|p| p.id == id);
            let position = p.map_or([0.; 3], |p| p.position);
            println!(
                "a2g-run: release t={:.3} id={id} kind {:?} mode {:?} spot-owner {} height {:.0} speed {:.1} stored {}",
                scene.seconds(),
                flight.kind,
                flight.mode,
                flight
                    .spot_owner
                    .map_or("none".to_owned(), |o| o.to_string()),
                position[1] - scene.ground(position[0], position[2]),
                p.map_or(0., |p| f64::from(p.speed_f8) / 256.),
                flight.point.map_or("none".to_owned(), |q| format!(
                    "{:.0} {:.0} {:.0}",
                    q[0], q[1], q[2]
                )),
            );
            followed.insert(
                id,
                Followed {
                    last: position,
                    flight: *flight,
                    released: t,
                    reported: 0,
                },
            );
        }
        // Each weapon in flight once a second, and its end.
        let target = scene.target_row().map(|r| (r.position, r.hp));
        let mut ended = Vec::new();
        for (id, f) in followed.iter_mut() {
            match (
                state.projectiles.iter().find(|p| p.id == *id),
                state.guided_flight(*id),
            ) {
                (Some(p), Some(flight)) => {
                    if flight.mode != f.flight.mode {
                        println!(
                            "a2g-run: mode t={:.3} id={id} {:?} -> {:?}",
                            scene.seconds(),
                            f.flight.mode,
                            flight.mode
                        );
                    }
                    f.last = p.position;
                    f.flight = *flight;
                    let second = (t - f.released) as u64;
                    if second >= f.reported {
                        f.reported = second + 1;
                        println!(
                            "a2g-run: flight t={:.3} id={id} mode {:?} height {:.0} speed {:.1} tti {}",
                            scene.seconds(),
                            flight.mode,
                            p.position[1] - scene.ground(p.position[0], p.position[2]),
                            f64::from(p.speed_f8) / 256.,
                            flight
                                .seconds
                                .map_or("none".to_owned(), |s| format!("{s:.1}")),
                        );
                    }
                }
                _ => ended.push(*id),
            }
        }
        for id in ended {
            let f = followed.remove(&id).expect("followed");
            let (miss, hp) = target.map_or((f64::NAN, -1), |(at, hp)| {
                (guided::ground_range(f.last, at), hp)
            });
            let stored = f
                .flight
                .point
                .map_or(f64::NAN, |q| guided::ground_range(f.last, q));
            println!(
                "a2g-run: impact t={:.3} id={id} mode {:?} at {:.0} {:.0} {:.0} target-miss {miss:.1} point-miss {stored:.1} target-hp {hp} flown {:.1}",
                scene.seconds(),
                f.flight.mode,
                f.last[0],
                f.last[1],
                f.last[2],
                t - f.released,
            );
        }
        if released.is_some_and(|r| t > r + 1.) && followed.is_empty() {
            break;
        }
    }
    println!(
        "a2g-run: summary released {} rounds {}",
        if released.is_some() { "yes" } else { "no" },
        scene.rounds()
    );
    scene.finish()?;
    println!("a2g-run: done");
    Ok(())
}

fn print_sight(label: &str, scene: &Scene, sight: &GuidedSight) {
    let state = &scene.world.combat.state;
    let laser = state
        .ownship(scene.owner)
        .map_or(LaserState::Off, |own| own.laser.state);
    println!(
        "a2g-run: {label} t={:.3} kind {:?} range {} reach {} in-reach {} advisory {} buddy {} laser {laser:?} bar {} along {} seconds {} steer {}",
        scene.seconds(),
        sight.kind,
        sight
            .ground_range_ft
            .map_or("none".to_owned(), |r| format!("{r:.0}")),
        sight.reach.map_or("none".to_owned(), |r| format!(
            "{:.0}..{:.0}",
            r.minimum_ft, r.maximum_ft
        )),
        if sight.in_reach { "yes" } else { "no" },
        sight
            .advisory
            .map_or("none".to_owned(), |a| a.label().replace(' ', "-")),
        if sight.buddy { "yes" } else { "no" },
        sight
            .release_range_ft()
            .map_or("none".to_owned(), |r| format!("{r:.0}")),
        sight
            .along_ft()
            .map_or("none".to_owned(), |r| format!("{r:.0}")),
        sight
            .release_seconds()
            .filter(|_| sight.level.is_some())
            .map_or("none".to_owned(), |r| format!("{r:.1}")),
        sight.level.map_or("none".to_owned(), |l| format!(
            "{:.1}",
            l.steering_rad.to_degrees()
        )),
    );
}
