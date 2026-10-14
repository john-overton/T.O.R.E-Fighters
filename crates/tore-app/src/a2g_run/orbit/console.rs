//! The sensor console's search and its keys (slice nr-cona), flown through
//! the player's path: `--orbit-sight free` opens the console with the sight
//! free, which searches straight and level, and `--console-keys T:KEY,...`
//! sends the console's keys as seat commands at T seconds: `pin` (P, which
//! pins the ground under the crosshair or lets it go), `heading+N` or
//! `heading-N`, `altitude+N` or `altitude-N` (steps), `speed=P` (percent of
//! the band) and `speed+N` or `speed-N` (steps).
//!
//! Checks: the console opens searching with nothing pinned; every search
//! settles on its commanded heading (and, from 15 s on, stays within 3
//! degrees once there), altitude and speed, wings level; every pin joins the orbit
//! and, after entry, every gun and the camera bear 90% of the time; the bank
//! and pitch rates stay smooth and there is no stall.
use super::{BANK_RATE_LIMIT, BEARING_SHARE, PITCH_RATE_LIMIT, Report};
use crate::AppResult;
use crate::a2g_run::{FPS_PER_KNOT, Scene};
use tore_sim::autopilot::orbit::{ConsoleCommand, ConsolePhase};
use tore_sim::combat::gunship::Sight;
use tore_world::seats::SeatCommand;

/// A search counts as settled within this of its heading, degrees...
const SETTLED_DEG: f64 = 1.;
/// ...and must then stay within this of it.
const HELD_DEG: f64 = 3.;
/// A search counts as settled no sooner than this after it starts, s.
const ROLL_OUT_S: f64 = 15.;
/// A search's last check is this long after its last key, s.
const SETTLE_S: f64 = 60.;

/// The `--console-keys` schedule: the console's keys at their times, and the
/// times a 4 s full-rate slew right starts (`T:slew`).
#[derive(Default)]
pub(super) struct Schedule {
    pub keys: Vec<(f64, ConsoleCommand)>,
    pub slews: Vec<f64>,
}

/// A slew lasts this long, s.
const SLEW_S: f64 = 4.;
/// While gun-locked, every pipper stays this close to the locked point, ft.
const LOCK_FT: f64 = 300.;

/// Parses `--console-keys`.
pub(super) fn keys(text: &str) -> AppResult<Schedule> {
    let mut out = Vec::new();
    let mut slews = Vec::new();
    for item in text.split(',').filter(|s| !s.is_empty()) {
        let (at, key) = item
            .split_once(':')
            .ok_or_else(|| format!("--console-keys item {item}: want T:KEY"))?;
        let at: f64 = at.parse()?;
        let steps = |rest: &str| -> AppResult<i8> { Ok(rest.parse::<i8>()?) };
        let command = if key == "pin" {
            ConsoleCommand::Pin
        } else if key == "gunlock" {
            ConsoleCommand::GunLock
        } else if key == "slew" {
            slews.push(at);
            continue;
        } else if let Some(rest) = key.strip_prefix("heading") {
            ConsoleCommand::Heading(steps(rest.trim_start_matches('+'))?)
        } else if let Some(rest) = key.strip_prefix("altitude") {
            ConsoleCommand::Altitude(steps(rest.trim_start_matches('+'))?)
        } else if let Some(rest) = key.strip_prefix("speed=") {
            ConsoleCommand::Speed(rest.parse()?)
        } else if let Some(rest) = key.strip_prefix("speed") {
            ConsoleCommand::SpeedStep(steps(rest.trim_start_matches('+'))?)
        } else {
            return Err(format!("--console-keys: unknown key {key}").into());
        };
        out.push((at, command));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(Schedule { keys: out, slews })
}

#[derive(Default)]
struct Search {
    /// Last key or phase change, s.
    since: f64,
    settled: bool,
    worst_after: f64,
    /// The heading the search took when the pin went (a roll-out).
    rollout: Option<f64>,
}

pub(super) fn run(scene: &mut Scene, seconds: f64, plan: &Schedule) -> AppResult<()> {
    let schedule = &plan.keys;
    let mut report = Report::default();
    scene.queue(SeatCommand::SensorConsole);
    scene.step(false)?;
    let sight = |scene: &Scene| {
        scene
            .world
            .combat
            .state
            .own()
            .gunship
            .as_ref()
            .map(|g| g.sight)
    };
    let flight = &scene.world.cockpits[0].flight;
    let status = flight.console_status();
    report.check(
        status.is_some_and(|s| s.phase == ConsolePhase::Search),
        "the console opened searching",
    );
    report.check(
        sight(scene) == Some(Sight::Free),
        "opening the console pinned nothing",
    );
    let mut next = 0;
    let mut search = Search::default();
    let mut last_phase = ConsolePhase::Search;
    let mut last: Option<(f64, f64)> = None;
    let (mut bank_rate, mut pitch_rate, mut stalled) = (0_f64, 0_f64, 0_u64);
    let (mut joined, mut entries, mut after, mut bearing) = (0, 0, 0_u64, 0_u64);
    let mut entered = false;
    let mut rejoin = false;
    let mut lock = Lock::default();
    let ticks = (seconds * 120.) as u64;
    for tick in 0..ticks {
        let t = tick as f64 / 120.;
        while next < schedule.len() && schedule[next].0 <= t {
            scene.queue(SeatCommand::Console(schedule[next].1));
            println!("orbit-run: t={t:.1} key {:?}", schedule[next].1);
            // A heading, altitude or speed key starts the search over; the
            // pin is checked at the phase change.
            if !matches!(
                schedule[next].1,
                ConsoleCommand::Pin | ConsoleCommand::GunLock
            ) {
                search.since = t;
                search.settled = false;
                search.worst_after = 0.;
            }
            next += 1;
        }
        let slewing = plan.slews.iter().any(|at| t >= *at && t < at + SLEW_S);
        if slewing {
            let input = tore_world::seats::SeatInput {
                seat: crate::a2g_run::SEAT,
                tick: scene.world.tick(),
                sight: [127, 0],
                commands: std::mem::take(&mut scene.commands),
                ..tore_world::seats::SeatInput::default()
            };
            scene.step_input(input)?;
        } else {
            scene.step(false)?;
        }
        lock.watch(scene, t, plan);
        let flight = &scene.world.cockpits[0].flight;
        report.fatal(!flight.crashed, "the aircraft did not crash")?;
        let Some(status) = flight.console_status() else {
            report.check(false, format_args!("the console closed at {t:.1} s"));
            break;
        };
        let bank = flight.bank.to_degrees();
        let pitch = flight.pitch.to_degrees();
        if let Some((b, p)) = last {
            bank_rate = bank_rate.max((bank - b).abs() * 120.);
            pitch_rate = pitch_rate.max((pitch - p).abs() * 120.);
        }
        last = Some((bank, pitch));
        if flight
            .stall_alert(scene.ground(flight.position[0], flight.position[2]))
            .is_some()
        {
            stalled += 1;
        }
        let searching = status.phase == ConsolePhase::Search;
        let was_searching = last_phase == ConsolePhase::Search;
        if searching != was_searching {
            if searching {
                // Released: the heading taken is the roll-out's.
                search = Search {
                    since: t,
                    rollout: Some(status.heading_deg),
                    ..Search::default()
                };
                println!(
                    "orbit-run: t={t:.1} released: search heading {:.1} (the heading at release, {bank:.1} deg of bank)",
                    status.heading_deg
                );
            } else {
                finish_search(&mut report, &search, scene, t, false);
                joined += 1;
                entered = false;
                rejoin = false;
                println!("orbit-run: t={t:.1} pinned: joining");
            }
        }
        last_phase = status.phase;
        if searching {
            let error =
                tore_sim::autopilot::orbit::heading_degrees(flight.yaw) - status.heading_deg;
            let error = (error + 180.).rem_euclid(360.) - 180.;
            // A release takes the current heading while still banked: the
            // roll-out turns a few degrees past it and the search turns back
            // (John, 2026-10-14), so a search counts as settled from
            // [`ROLL_OUT_S`] after its start.
            if error.abs() < SETTLED_DEG && t - search.since >= ROLL_OUT_S {
                search.settled = true;
            }
            if search.settled {
                search.worst_after = search.worst_after.max(error.abs());
            }
        } else if let Some(orbit) = status.orbit {
            // A new point (the orbit back to JOIN) is entered again, by the
            // same test, before its bearing counts.
            if entered && status.phase == ConsolePhase::Join {
                entered = false;
                rejoin = true;
                println!("orbit-run: t={t:.1} joining a new point");
            }
            if !entered && orbit.on_orbit && (orbit.height_now_ft - orbit.height_ft).abs() < 500. {
                entered = true;
                if !rejoin {
                    entries += 1;
                }
                println!("orbit-run: t={t:.1} entry");
            }
            // On the circle (a new point, such as the sight's after a gun
            // unlock, is joined again first).
            // The share is the steady orbit's: a run that unlocks the guns
            // onto a point a mile away stops counting there (the re-join
            // passes over the old circle).
            if entered && status.phase == ConsolePhase::Orbit && lock.unlocked_at.is_none() {
                after += 1;
                bearing += u64::from(orbit.all_bearing());
            }
        }
        if tick % (120 * 10) == 0 {
            println!(
                "orbit-run: t={t:.0} {} heading {:.1} (cmd {:.1}) alt {:.0} (cmd {:.0}) speed {:.0} kt (cmd {:.0}) bank {bank:.1}",
                status.phase.label(),
                tore_sim::autopilot::orbit::heading_degrees(flight.yaw),
                status.heading_deg,
                flight.position[1],
                status.altitude_ft,
                flight.speed / FPS_PER_KNOT,
                status.speed_kt
            );
        }
    }
    if last_phase == ConsolePhase::Search {
        finish_search(&mut report, &search, scene, seconds, true);
    }
    let pins = schedule
        .iter()
        .filter(|(_, c)| *c == ConsoleCommand::Pin)
        .count();
    report.check(
        joined == pins.div_ceil(2),
        format_args!("every pin joined an orbit ({joined})"),
    );
    report.check(
        entries == joined,
        format_args!("every orbit was entered ({entries} of {joined})"),
    );
    if after > 0 {
        let share = bearing as f64 / after as f64;
        report.check(
            share >= BEARING_SHARE,
            format_args!(
                "every gun and the camera bear at least {:.0}% of the time after entry ({:.1}%)",
                BEARING_SHARE * 100.,
                share * 100.
            ),
        );
    }
    lock.finish(&mut report, schedule);
    report.check(stalled == 0, "no stall warning");
    report.check(
        bank_rate <= BANK_RATE_LIMIT,
        format_args!("bank rate at most {BANK_RATE_LIMIT} deg/s ({bank_rate:.1})"),
    );
    report.check(
        pitch_rate <= PITCH_RATE_LIMIT,
        format_args!("pitch rate at most {PITCH_RATE_LIMIT} deg/s ({pitch_rate:.1})"),
    );
    report.done()
}

/// The checks at the end of a search: settled on the commanded heading and
/// held there, and, given the time, on the altitude and speed, wings level.
fn finish_search(report: &mut Report, search: &Search, scene: &Scene, t: f64, end: bool) {
    let flight = &scene.world.cockpits[0].flight;
    let Some(status) = flight.console_status() else {
        return;
    };
    let what = if search.rollout.is_some() {
        "the search after the release"
    } else {
        "the search"
    };
    report.check(
        search.settled && search.worst_after < HELD_DEG,
        format_args!(
            "{what} settled on its heading and held it within {HELD_DEG} degrees ({:.1}) by {t:.0} s",
            search.worst_after
        ),
    );
    if end && t - search.since >= SETTLE_S {
        let ground = scene.ground(flight.position[0], flight.position[2]);
        let altitude = status
            .altitude_ft
            .max(ground + tore_sim::autopilot::orbit::TERRAIN_FLOOR_FT);
        report.check(
            (flight.position[1] - altitude).abs() < 200.,
            format_args!(
                "{what} holds its altitude {altitude:.0} ({:.0})",
                flight.position[1]
            ),
        );
        report.check(
            (flight.speed / FPS_PER_KNOT - status.speed_kt).abs() < 8.,
            format_args!(
                "{what} holds its speed {:.0} kt ({:.0})",
                status.speed_kt,
                flight.speed / FPS_PER_KNOT
            ),
        );
        report.check(
            flight.bank.to_degrees().abs() < 3.,
            format_args!(
                "{what} flies wings level ({:.1} deg)",
                flight.bank.to_degrees()
            ),
        );
    }
}

/// The gun lock as the run sees it.
#[derive(Default)]
struct Lock {
    /// Ticks locked after a slew ended, the worst pipper distance from the
    /// locked point then, the farthest the sight looked from it, and the
    /// ticks the orbit circled anything else.
    ticks: u64,
    worst_ft: f64,
    sight_ft: f64,
    orbit_off: u64,
    /// Unlocked after a lock: when, and the worst pipper distance from the
    /// sight's aim from 20 s after.
    unlocked_at: Option<f64>,
    after_ticks: u64,
    after_worst_ft: f64,
}
impl Lock {
    fn watch(&mut self, scene: &Scene, t: f64, plan: &Schedule) {
        let Some(guns) = scene.world.combat.state.own().gunship.as_ref() else {
            return;
        };
        let far = |a: [f64; 3], b: [f64; 3]| (a[0] - b[0]).hypot(a[2] - b[2]);
        let slewed = plan.slews.iter().any(|at| t >= at + SLEW_S + 1.);
        match guns.gun_lock {
            Some(lock) if slewed => {
                self.ticks += 1;
                for impact in guns.impacts.iter().flatten() {
                    self.worst_ft = self.worst_ft.max(far(impact.point(), lock.point));
                }
                if let Some(aim) = guns.aim {
                    self.sight_ft = self.sight_ft.max(far(aim, lock.point));
                }
                let status = scene.world.cockpits[0].flight.orbit_status();
                if status.is_none_or(|o| far(o.point, lock.point) > 1.) {
                    self.orbit_off += 1;
                }
            }
            Some(_) => {}
            None if self.ticks > 0 => {
                let at = *self.unlocked_at.get_or_insert(t);
                if t >= at + 20.
                    && let Some(aim) = guns.aim
                {
                    self.after_ticks += 1;
                    for impact in guns.impacts.iter().flatten() {
                        self.after_worst_ft = self.after_worst_ft.max(far(impact.point(), aim));
                    }
                }
            }
            None => {}
        }
    }
    fn finish(&self, report: &mut Report, schedule: &[(f64, ConsoleCommand)]) {
        if !schedule.iter().any(|(_, c)| *c == ConsoleCommand::GunLock) {
            return;
        }
        println!(
            "orbit-run: gun lock {:.1} s after the slew: pippers within {:.0} ft of the locked point, the sight up to {:.0} ft from it; after unlock pippers within {:.0} ft of the sight",
            self.ticks as f64 / 120.,
            self.worst_ft,
            self.sight_ft,
            self.after_worst_ft
        );
        report.check(
            self.ticks > 0,
            "the guns were locked while the sight slewed away",
        );
        report.check(
            self.sight_ft > 1_000.,
            format_args!(
                "the sight looked away from the locked point ({:.0} ft)",
                self.sight_ft
            ),
        );
        report.check(
            self.worst_ft <= LOCK_FT,
            format_args!(
                "the pippers stayed within {LOCK_FT} ft of the locked point ({:.0})",
                self.worst_ft
            ),
        );
        report.check(
            self.orbit_off == 0,
            "the orbit circled the locked point, not the sight's",
        );
        if self.unlocked_at.is_some() {
            report.check(
                self.after_ticks > 0 && self.after_worst_ft <= LOCK_FT,
                format_args!(
                    "unlocked, the pippers went back to the sight ({:.0} ft)",
                    self.after_worst_ft
                ),
            );
        }
    }
}
