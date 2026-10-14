//! The sensor console's search and its keys (slice nr-cona), flown through
//! the player's path: `--orbit-sight free` opens the console with the sight
//! free, which searches straight and level, and `--console-keys T:KEY,...`
//! sends the console's keys as seat commands at T seconds: `pin` (P, which
//! pins the ground under the crosshair or lets it go), `heading+N` or
//! `heading-N`, `altitude+N` or `altitude-N` (steps), `speed=P` (percent of
//! the band) and `speed+N` or `speed-N` (steps).
//!
//! Checks: the console opens searching with nothing pinned; every search
//! settles on its commanded heading (and stays within 3 degrees once there,
//! no turn back), altitude and speed, wings level; every pin joins the orbit
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
/// A search's last check is this long after its last key, s.
const SETTLE_S: f64 = 60.;

/// The `--console-keys` schedule: (seconds, command).
pub(super) fn keys(text: &str) -> AppResult<Vec<(f64, ConsoleCommand)>> {
    let mut out = Vec::new();
    for item in text.split(',').filter(|s| !s.is_empty()) {
        let (at, key) = item
            .split_once(':')
            .ok_or_else(|| format!("--console-keys item {item}: want T:KEY"))?;
        let at: f64 = at.parse()?;
        let steps = |rest: &str| -> AppResult<i8> { Ok(rest.parse::<i8>()?) };
        let command = if key == "pin" {
            ConsoleCommand::Pin
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
    Ok(out)
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

pub(super) fn run(
    scene: &mut Scene,
    seconds: f64,
    schedule: &[(f64, ConsoleCommand)],
) -> AppResult<()> {
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
    let ticks = (seconds * 120.) as u64;
    for tick in 0..ticks {
        let t = tick as f64 / 120.;
        while next < schedule.len() && schedule[next].0 <= t {
            scene.queue(SeatCommand::Console(schedule[next].1));
            println!("orbit-run: t={t:.1} key {:?}", schedule[next].1);
            search.since = t;
            search.settled = false;
            search.worst_after = 0.;
            next += 1;
        }
        scene.step(false)?;
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
                    "orbit-run: t={t:.1} released: search heading {:.1} (roll-out from {bank:.1} deg of bank)",
                    status.heading_deg
                );
            } else {
                finish_search(&mut report, &search, scene, t, false);
                joined += 1;
                entered = false;
                println!("orbit-run: t={t:.1} pinned: joining");
            }
        }
        last_phase = status.phase;
        if searching {
            let error =
                tore_sim::autopilot::orbit::heading_degrees(flight.yaw) - status.heading_deg;
            let error = (error + 180.).rem_euclid(360.) - 180.;
            if error.abs() < SETTLED_DEG {
                search.settled = true;
            }
            if search.settled {
                search.worst_after = search.worst_after.max(error.abs());
            }
        } else if let Some(orbit) = status.orbit {
            if !entered && orbit.on_orbit && (orbit.height_now_ft - orbit.height_ft).abs() < 500. {
                entered = true;
                entries += 1;
                println!("orbit-run: t={t:.1} entry");
            }
            if entered {
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
