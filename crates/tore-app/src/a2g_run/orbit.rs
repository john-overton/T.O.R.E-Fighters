//! The `orbit` attack (slice nr-orbit): the AC-130's sensor console and its
//! orbit hold, flown through the player's path (docs/spec/ac130-linked-guns.md,
//! "The sensor console and the orbit"). No stick, no throttle: the run opens
//! the console with its seat command and watches the autopilot fly.
//!
//! `--orbit-sight pin|track|free` says what the sight holds when the console
//! opens: the ground under the target pinned or the target tracked (both set
//! by hand, setup shortcuts), or a free sight, with which the console
//! searches (slice nr-cona, [`console`]; `--console-keys` sends its keys).
//! `--orbit-slew S` holds the sight's slew
//! right at full deflection for 4 s from S seconds, which moves a pin (the pod
//! point moves); `--orbit-close S` closes the console at S seconds.
//!
//! Each line starts `orbit-run:`; checks print `PASS` or `FAIL` lines and a
//! last `orbit probe: PASS` the battery reads.
use super::{FEET_PER_NM, FPS_PER_KNOT, Options, Scene};
use crate::AppResult;
use tore_sim::autopilot::Mode;
use tore_sim::combat::gunship::Sight;
use tore_world::seats::{SeatCommand, SeatInput};

/// The options this module reads, each with one value.
pub(super) const OPTIONS: [&str; 4] = [
    "--orbit-sight",
    "--orbit-slew",
    "--orbit-close",
    "--console-keys",
];
/// The share of the time after entry that every gun and the camera must
/// bear on the sight's point (fitted acceptance, agent decision).
const BEARING_SHARE: f64 = 0.9;
/// The bank and pitch rates the run accepts, degrees a second: the bank
/// command slews at 5, the flight model adds a little.
const BANK_RATE_LIMIT: f64 = 8.;
const PITCH_RATE_LIMIT: f64 = 4.;
/// The slew is held this long, s.
const SLEW_SECONDS: f64 = 4.;

mod console;

struct Extra {
    sight: String,
    slew: Option<f64>,
    close: Option<f64>,
    keys: Vec<(f64, tore_sim::autopilot::orbit::ConsoleCommand)>,
}

fn extra() -> AppResult<Extra> {
    let args: Vec<String> = std::env::args().collect();
    let mut e = Extra {
        sight: "pin".into(),
        slew: None,
        close: None,
        keys: Vec::new(),
    };
    for pair in args.windows(2) {
        let value = &pair[1];
        match pair[0].as_str() {
            "--orbit-sight" => match value.as_str() {
                "pin" | "track" | "free" => e.sight = value.clone(),
                other => return Err(format!("unknown --orbit-sight {other}").into()),
            },
            "--orbit-slew" => e.slew = Some(value.parse()?),
            "--orbit-close" => e.close = Some(value.parse()?),
            "--console-keys" => e.keys = console::keys(value)?,
            _ => {}
        }
    }
    Ok(e)
}

#[derive(Default)]
struct Watch {
    /// First tick on the orbit, s.
    entry: Option<f64>,
    after: u64,
    bearing: u64,
    guns: [u64; 3],
    sensor: u64,
    min_distance: f64,
    min_height: f64,
    max_bank: f64,
    max_bank_rate: f64,
    max_pitch_rate: f64,
    stalled: u64,
    last: Option<(f64, f64)>,
    speed_sum: f64,
}

pub(super) fn run(scene: &mut Scene, o: &Options) -> AppResult<()> {
    let e = extra()?;
    let row = scene.target_row().ok_or("no target row")?;
    let target = row.position;
    let ground = scene.ground(target[0], target[2]);
    // Start `from` miles out on the reciprocal of `heading`, flying it.
    let heading = o.heading_deg.to_radians();
    let distance = o.from_nm * FEET_PER_NM;
    let start = [
        target[0] - heading.sin() * distance,
        ground + o.altitude,
        target[2] - heading.cos() * distance,
    ];
    scene.fly(start, heading, 0., o.speed_kt * FPS_PER_KNOT);
    scene.world.cockpits[0].flight.throttle = 0.7;
    if e.sight != "free" {
        let id = scene.target;
        let guns = scene
            .world
            .combat
            .state
            .own_mut()
            .gunship
            .as_mut()
            .ok_or("the orbit run needs the AC-130 (--aircraft ac130)")?;
        // Setup shortcuts: the sight's keys are the gunsight probes' to test.
        if e.sight == "pin" {
            guns.sight = Sight::Pinned([target[0], ground, target[2]]);
        } else {
            guns.track(id);
        }
    }
    println!(
        "orbit-run: start {:.0} {:.0} {:.0} target {:.0} {:.0} {:.0} sight {}",
        start[0], start[1], start[2], target[0], ground, target[2], e.sight
    );
    // Half a second for the designation and the sight to settle.
    for _ in 0..60 {
        scene.step(false)?;
    }
    if e.sight == "free" {
        return console::run(scene, o.seconds, &e.keys);
    }
    scene.queue(SeatCommand::SensorConsole);
    scene.step(false)?;
    let flight = &scene.world.cockpits[0].flight;
    let mut report = Report::default();
    report.check(
        flight.autopilot.mode() == Mode::Orbit,
        "the console opened orbit hold",
    );
    let Some(status) = flight.orbit_status() else {
        return report.done();
    };
    println!(
        "orbit-run: plan radius {:.0} height {:.0} speed {:.0} kt minimum radius {:.0} height {:.0} point {:.0} {:.0} {:.0}",
        status.radius_ft,
        status.height_ft,
        status.speed_fps / FPS_PER_KNOT,
        status.minimum_radius_ft,
        status.minimum_height_ft,
        status.point[0],
        status.point[1],
        status.point[2]
    );
    report.check(
        status.radius_ft >= status.minimum_radius_ft
            && status.height_ft >= status.minimum_height_ft,
        format_args!(
            "the plan ({:.0} ft out, {:.0} ft up) is not below the minimum ({:.0}, {:.0})",
            status.radius_ft, status.height_ft, status.minimum_radius_ft, status.minimum_height_ft
        ),
    );
    let pinned = matches!(
        scene
            .world
            .combat
            .state
            .own()
            .gunship
            .as_ref()
            .map(|g| g.sight),
        Some(Sight::Pinned(_) | Sight::Tracked(_))
    );
    report.check(pinned, "the sight holds a pin or a track in the console");
    let mut w = Watch {
        min_distance: f64::INFINITY,
        min_height: f64::INFINITY,
        ..Watch::default()
    };
    let first_point = status.point;
    let ticks = (o.seconds * 120.) as u64;
    for tick in 0..ticks {
        let t = tick as f64 / 120.;
        let slewing = e.slew.is_some_and(|at| t >= at && t < at + SLEW_SECONDS);
        if e.close.is_some_and(|at| (t - at).abs() < 0.5 / 120.) {
            scene.queue(SeatCommand::SensorConsole);
        }
        step(scene, slewing)?;
        // `--console-scene DIR --console-at S,...`: the sensor console's
        // frames from this seat (slice nr-fltv).
        if let Some(dir) = &o.ai.console_scene
            && o.ai.console_at.iter().any(|at| (t - at).abs() < 0.5 / 120.)
        {
            super::console_render::render_seat(scene, t, dir)?;
        }
        let flight = &scene.world.cockpits[0].flight;
        report.fatal(!flight.crashed, "the aircraft did not crash")?;
        let Some(s) = flight.orbit_status() else {
            if e.close.is_some() {
                break;
            }
            report.check(false, format_args!("orbit hold dropped at {t:.1} s"));
            break;
        };
        let bank = flight.bank.to_degrees();
        let pitch = flight.pitch.to_degrees();
        if let Some((b, p)) = w.last {
            w.max_bank_rate = w.max_bank_rate.max((bank - b).abs() * 120.);
            w.max_pitch_rate = w.max_pitch_rate.max((pitch - p).abs() * 120.);
        }
        w.last = Some((bank, pitch));
        if w.entry.is_none() && s.on_orbit && (s.height_now_ft - s.height_ft).abs() < 500. {
            w.entry = Some(t);
            println!("orbit-run: entry at {t:.1} s");
        }
        if w.entry.is_some() {
            w.after += 1;
            w.bearing += u64::from(s.all_bearing());
            for (count, on) in w.guns.iter_mut().zip(s.guns_bearing) {
                *count += u64::from(on);
            }
            w.sensor += u64::from(s.sensor_bearing);
            w.min_distance = w.min_distance.min(s.distance_ft);
            w.min_height = w.min_height.min(s.height_now_ft);
            w.max_bank = w.max_bank.max(bank.abs());
            w.speed_sum += flight.speed;
        }
        if flight
            .stall_alert(scene.ground(flight.position[0], flight.position[2]))
            .is_some()
        {
            w.stalled += 1;
        }
        if tick % (120 * 10) == 0 {
            println!(
                "orbit-run: t={t:.0} distance {:.0} height {:.0} bank {bank:.1} pitch {pitch:.1} speed {:.0} kt throttle {:.2} on {} bearing {:?} {} slant {:.0}",
                s.distance_ft,
                s.height_now_ft,
                flight.speed / FPS_PER_KNOT,
                flight.throttle,
                s.on_orbit,
                s.guns_bearing,
                s.sensor_bearing,
                s.slant_ft
            );
        }
    }
    finish(scene, &e, &mut report, &w, first_point, status)
}

fn finish(
    scene: &Scene,
    e: &Extra,
    report: &mut Report,
    w: &Watch,
    first_point: tore_sim::attitude::Vector,
    status: tore_sim::autopilot::orbit::OrbitStatus,
) -> AppResult<()> {
    let after = w.after.max(1) as f64;
    println!(
        "orbit-run: after entry {:.0} s all-bearing {:.3} guns {:.3} {:.3} {:.3} sensor {:.3} min distance {:.0} min height {:.0} max bank {:.1} max bank rate {:.1} max pitch rate {:.1} mean speed {:.0} kt stalled ticks {}",
        w.after as f64 / 120.,
        w.bearing as f64 / after,
        w.guns[0] as f64 / after,
        w.guns[1] as f64 / after,
        w.guns[2] as f64 / after,
        w.sensor as f64 / after,
        w.min_distance,
        w.min_height,
        w.max_bank,
        w.max_bank_rate,
        w.max_pitch_rate,
        w.speed_sum / after / FPS_PER_KNOT,
        w.stalled
    );
    report.check(w.entry.is_some(), "the aircraft entered the orbit");
    report.check(w.stalled == 0, "no stall warning");
    report.check(
        w.max_bank_rate <= BANK_RATE_LIMIT,
        format_args!(
            "bank rate at most {BANK_RATE_LIMIT} deg/s ({:.1})",
            w.max_bank_rate
        ),
    );
    report.check(
        w.max_pitch_rate <= PITCH_RATE_LIMIT,
        format_args!(
            "pitch rate at most {PITCH_RATE_LIMIT} deg/s ({:.1})",
            w.max_pitch_rate
        ),
    );
    if let Some(close) = e.close {
        let flight = &scene.world.cockpits[0].flight;
        report.check(
            flight.autopilot.mode() == Mode::Off && !flight.crashed,
            format_args!("the console closed at {close} s and handed the aircraft back"),
        );
        return report.done();
    }
    if w.entry.is_some() {
        report.check(
            w.max_bank <= 31.,
            format_args!("bank at most 31 degrees after entry ({:.1})", w.max_bank),
        );
        report.check(
            w.min_distance >= status.minimum_radius_ft,
            format_args!(
                "never inside the minimum radius {:.0} after entry ({:.0})",
                status.minimum_radius_ft, w.min_distance
            ),
        );
        report.check(
            w.min_height >= status.minimum_height_ft,
            format_args!(
                "never below the minimum height {:.0} after entry ({:.0})",
                status.minimum_height_ft, w.min_height
            ),
        );
        report.check(
            w.bearing as f64 / after >= BEARING_SHARE,
            format_args!(
                "every gun and the camera bear at least {:.0}% of the time after entry ({:.1}%)",
                BEARING_SHARE * 100.,
                w.bearing as f64 / after * 100.
            ),
        );
    }
    if e.slew.is_some() || e.sight == "track" {
        let now = scene.world.cockpits[0]
            .flight
            .orbit_status()
            .map_or(first_point, |s| s.point);
        let moved = (now[0] - first_point[0]).hypot(now[2] - first_point[2]);
        println!("orbit-run: the point moved {moved:.0} ft");
        report.check(
            moved > 300.,
            "the pod point moved and the orbit followed it",
        );
    }
    report.done()
}

/// One tick with no stick or throttle, the sight slewed right when asked.
fn step(scene: &mut Scene, slewing: bool) -> AppResult<()> {
    if !slewing {
        return scene.step(false);
    }
    let input = SeatInput {
        seat: super::SEAT,
        tick: scene.world.tick(),
        sight: [127, 0],
        commands: std::mem::take(&mut scene.commands),
        ..SeatInput::default()
    };
    scene.step_input(input)
}

#[derive(Default)]
struct Report {
    failures: usize,
}
impl Report {
    fn check(&mut self, ok: bool, what: impl std::fmt::Display) {
        if ok {
            println!("PASS {what}");
        } else {
            println!("FAIL {what}");
            self.failures += 1;
        }
    }
    fn fatal(&mut self, ok: bool, what: &str) -> AppResult<()> {
        if ok {
            return Ok(());
        }
        self.check(false, what);
        self.done()
    }
    fn done(&self) -> AppResult<()> {
        if self.failures == 0 {
            println!("orbit probe: PASS");
            Ok(())
        } else {
            Err(format!("orbit probe: {} check(s) FAILED", self.failures).into())
        }
    }
}
