//! The `--gunsight-probe` command-line check: scripted AC-130 gunsight runs on
//! flat ground through the live combat state, with the real imported guns.
//! They print `PASS` and `FAIL` lines the battery reads, and
//! `--gunsight-dump PATH` writes the per-tick gun train of an orbit as CSV.
//! Nothing here opens a window or reads a clock.
// The three guns' parallel arrays are read slot by slot; ranges read plainer.
#![allow(clippy::needless_range_loop)]
use crate::AppResult;
use std::{f64::consts::PI, fmt::Write as _};
use tore_sim::{
    attitude::Basis,
    combat::{
        gunship::{self, Sight},
        live::{
            self, Command, Configuration, EffectKind, Event, Launcher, OwnshipInput, Readiness,
        },
    },
    sensors,
};

type Vector = [f64; 3];

/// The probes by name, as the battery spells them.
pub const NAMES: [&str; 3] = ["pin-orbit", "fire-no-target", "track-out-of-arc"];

const GRAVITY: f64 = 32.174;
const HZ: f64 = 120.;
/// One tick of the 30 degrees a second a gun may slew, in radians, with a
/// tenth over for the rounding of a quantized step.
const MAX_STEP: f64 = 30. * PI / 180. / HZ * 1.1;

/// Results collected for the report: `ok` lines and failures.
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
    fn done(self, name: &str) -> AppResult<()> {
        if self.failures == 0 {
            println!("gunsight probe {name}: PASS");
            Ok(())
        } else {
            Err(format!("gunsight probe {name}: {} check(s) FAILED", self.failures).into())
        }
    }
}

fn launcher(position: Vector, yaw: f64, bank: f64, speed: f64) -> Launcher {
    let basis = Basis::new(yaw, 0., bank);
    Launcher {
        radar_power: true,
        position,
        basis,
        speed_fps: speed,
        velocity: basis.forward.map(|v| v * speed),
        bay_ready: true,
        radar: true,
        jammer: false,
        alive: true,
        body_present: true,
        controls: sensors::Controls::default(),
    }
}

/// A left orbit of `radius` feet around ground point `centre` at `altitude`
/// feet: where the aircraft is `seconds` after it was due north of the centre
/// and heading west, with the bank that turns it.
fn orbit(centre: Vector, radius: f64, altitude: f64, speed: f64, seconds: f64) -> Launcher {
    // Heading falls as the aircraft turns left; the centre is on its left.
    let yaw = 0.75 * PI * 2. - speed / radius * seconds;
    let position = [
        centre[0] + radius * yaw.cos(),
        altitude,
        centre[2] - radius * yaw.sin(),
    ];
    let bank = -(speed * speed / (radius * GRAVITY)).atan();
    launcher(position, yaw, bank, speed)
}

fn flat(_x: f64, _z: f64) -> f64 {
    0.
}

fn offset(a: Vector, b: Vector) -> Vector {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn length(v: Vector) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}
fn horizontal(v: Vector) -> f64 {
    (v[0] * v[0] + v[2] * v[2]).sqrt()
}

/// An AC-130 state with all three guns linked and the first selected.
fn state(config: &Configuration) -> AppResult<live::State> {
    let mut s = live::State::new(config.clone(), true)?;
    let group = s
        .own_mut()
        .gunship
        .as_mut()
        .ok_or("the gunsight probe needs the AC-130 (--aircraft ac130)")?;
    group.included = [true; 3];
    let first = group
        .stations
        .iter()
        .flatten()
        .copied()
        .next()
        .ok_or("the AC-130 carries no guns")?;
    let own = s.own_mut();
    own.selected = first;
    own.armed = true;
    Ok(s)
}

fn group(s: &live::State) -> &gunship::State {
    s.own().gunship.as_ref().expect("an AC-130")
}
fn group_mut(s: &mut live::State) -> &mut gunship::State {
    s.own_mut().gunship.as_mut().expect("an AC-130")
}

fn tick(s: &mut live::State, l: Launcher, held: bool) -> Vec<Event> {
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held,
            launcher: l,
        }],
        flat,
    )
}

/// What a run of ticks counted.
#[derive(Default)]
struct Tally {
    shots: usize,
    ground: usize,
}
impl Tally {
    fn add(&mut self, events: &[Event]) {
        for e in events {
            match e {
                Event::Fired { .. } => self.shots += 1,
                Event::Ground => self.ground += 1,
                _ => {}
            }
        }
    }
}

/// Where rounds have struck the ground: each ground explosion the state has
/// shown, once (the AC-130's guns leave no crater marks, so the effect's
/// position is the impact).
#[derive(Default)]
struct Impacts {
    seen: std::collections::BTreeSet<[u64; 3]>,
    points: Vec<Vector>,
}
impl Impacts {
    fn collect(&mut self, s: &live::State) {
        for e in s.effects.iter().filter(|e| e.kind == EffectKind::Ground) {
            if self.seen.insert(e.position.map(f64::to_bits)) {
                self.points.push(e.position);
            }
        }
    }
    fn worst(&self, from: Vector) -> f64 {
        self.points
            .iter()
            .map(|p| horizontal(offset(*p, from)))
            .fold(0., f64::max)
    }
    fn mean(&self, from: Vector) -> f64 {
        if self.points.is_empty() {
            f64::NAN
        } else {
            self.points
                .iter()
                .map(|p| horizontal(offset(*p, from)))
                .sum::<f64>()
                / self.points.len() as f64
        }
    }
}

/// Runs one probe by name. `dump` is a CSV path for the orbit's gun train.
pub fn run(config: &Configuration, name: &str, dump: Option<&std::path::Path>) -> AppResult<()> {
    match name {
        "pin-orbit" => pin_orbit(config, dump),
        "fire-no-target" => fire_no_target(config),
        "track-out-of-arc" => track_out_of_arc(config),
        _ => Err(format!("--gunsight-probe needs one of {}", NAMES.join(", ")).into()),
    }
}

/// Orbit at 4,000 feet over flat ground, pin the ground at the orbit's centre
/// and fire for ten seconds. The rounds must land around the pin, the guns
/// must keep up, and the barrels must never jump.
fn pin_orbit(config: &Configuration, dump: Option<&std::path::Path>) -> AppResult<()> {
    let mut report = Report::default();
    let mut s = state(config)?;
    let centre = [0., 0., 0.];
    let (altitude, speed, radius) = (4000., 300., 6000.);
    let mut seconds = 0.;
    let mut l = orbit(centre, radius, altitude, speed, seconds);
    // Look at the centre, then pin: the pin resolves on the first step, along
    // the look, from the sensor dome.
    group_mut(&mut s).look = gunship::body_angles(l, offset(centre, gunship::eye_position(l)));
    s.command(0, Command::SightPinGround, l);
    tick(&mut s, l, false);
    let pin = match group(&s).sight {
        Sight::Pinned(p) => p,
        other => {
            report.check(
                false,
                format!("the sight pinned the ground (it holds {other:?})"),
            );
            return report.done("pin-orbit");
        }
    };
    report.check(
        length(offset(pin, centre)) < 25.,
        format!(
            "the pin is at the orbit's centre (off by {:.1} ft)",
            length(offset(pin, centre))
        ),
    );
    let fire_from = 4 * 120;
    let total = 16 * 120;
    let fire_until = 14 * 120;
    let mut tally = Tally::default();
    let mut csv = String::from(
        "tick,seconds,x,z,left_bank_deg,look_heading,look_elevation,aim_x,aim_z,\
         h25,e25,h40,e40,h105,e105,s25,s40,s105,sight\n",
    );
    let mut last = group(&s)
        .headings
        .iter()
        .zip(group(&s).elevations)
        .map(|(h, e)| (*h, e))
        .collect::<Vec<_>>();
    let mut biggest_step = 0f64;
    let mut ready_ticks = 0usize;
    let mut judged = 0usize;
    let mut held_pin = true;
    let mut worst_mask = 0usize;
    let mut impacts = Impacts::default();
    for t in 0..total {
        seconds = f64::from(t) / HZ;
        l = orbit(centre, radius, altitude, speed, seconds);
        let events = tick(&mut s, l, t >= fire_from && t < fire_until);
        tally.add(&events);
        impacts.collect(&s);
        let g = group(&s);
        held_pin &= matches!(g.sight, Sight::Pinned(p) if length(offset(p, pin)) < 1e-6);
        for slot in 0..3 {
            let (h, e) = (g.headings[slot], g.elevations[slot]);
            biggest_step = biggest_step
                .max((h - last[slot].0).abs())
                .max((e - last[slot].1).abs());
            last[slot] = (h, e);
        }
        if t >= fire_from {
            for slot in 0..3 {
                if g.stations[slot].is_some() && g.included[slot] {
                    judged += 1;
                    if g.status[slot] == Readiness::Ready {
                        ready_ticks += 1;
                    }
                    if g.status[slot] == Readiness::GunObscured {
                        worst_mask += 1;
                    }
                }
            }
        }
        if t % 12 == 0 {
            let aim = g.aim.unwrap_or([f64::NAN; 3]);
            let _ = writeln!(
                csv,
                "{t},{:.3},{:.1},{:.1},{:.2},{:.2},{:.2},{:.1},{:.1},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:?},{:?},{:?},pinned",
                f64::from(t) / HZ,
                l.position[0],
                l.position[2],
                l.basis.right[1].asin().to_degrees(),
                g.look[0].to_degrees(),
                g.look[1].to_degrees(),
                aim[0],
                aim[2],
                g.headings[0].to_degrees(),
                g.elevations[0].to_degrees(),
                g.headings[1].to_degrees(),
                g.elevations[1].to_degrees(),
                g.headings[2].to_degrees(),
                g.elevations[2].to_degrees(),
                g.status[0],
                g.status[1],
                g.status[2],
            );
        }
    }
    let (worst, mean) = (impacts.worst(pin), impacts.mean(pin));
    println!(
        "pin-orbit: shots={} ground_impacts={} distinct impact points={} distance from the pin mean={:.1} max={:.1} ft; \
         biggest barrel step {:.4} deg (limit {:.4}); ready {}/{} gun-ticks",
        tally.shots,
        tally.ground,
        impacts.points.len(),
        mean,
        worst,
        biggest_step.to_degrees(),
        MAX_STEP.to_degrees(),
        ready_ticks,
        judged
    );
    report.check(
        held_pin,
        "the sight held the same pinned point for the whole orbit",
    );
    report.check(
        tally.shots > 0,
        format!("the trigger fired ({} rounds)", tally.shots),
    );
    report.check(
        tally.ground > 0 && !impacts.points.is_empty(),
        format!(
            "rounds landed ({} ground impacts, {} distinct points)",
            tally.ground,
            impacts.points.len()
        ),
    );
    report.check(
        worst < 150.,
        format!("every ground impact is within 150 ft of the pin (worst {worst:.1} ft)"),
    );
    report.check(
        biggest_step <= MAX_STEP,
        format!(
            "no barrel moved faster than 30 degrees a second (biggest step {:.4} deg)",
            biggest_step.to_degrees()
        ),
    );
    report.check(
        judged > 0 && ready_ticks * 100 >= judged * 95,
        format!("the linked guns read READY on {ready_ticks} of {judged} gun-ticks while firing"),
    );
    report.check(
        worst_mask == 0,
        "no gun read NO LINE OF FIRE during the orbit",
    );
    if let Some(path) = dump {
        std::fs::write(path, csv)?;
        println!("orbit dump written to {}", path.display());
    }
    report.done("pin-orbit")
}

/// Level flight at 3,000 feet, nothing designated and nothing pinned: the
/// default view, and the trigger must still fire all three guns and leave
/// craters where the line of sight meets the ground.
fn fire_no_target(config: &Configuration) -> AppResult<()> {
    let mut report = Report::default();
    let mut s = state(config)?;
    let l = launcher([0., 3000., 0.], 0., 0., 300.);
    // The guns come round to the default view first.
    for _ in 0..180 {
        tick(&mut s, l, false);
    }
    let g = group(&s);
    report.check(
        matches!(g.sight, Sight::Free),
        "nothing is held: the sight is in free slew",
    );
    report.check(
        (g.look[0] - gunship::DEFAULT_LOOK[0]).abs() < 1e-9
            && (g.look[1] - gunship::DEFAULT_LOOK[1]).abs() < 1e-9,
        "the sight is in the default view (90 degrees left, 25 down)",
    );
    let aim = g.aim.ok_or("no aim point in free slew over flat ground")?;
    let mut tally = Tally::default();
    let mut slots_fired = [false; 3];
    let mut impacts = Impacts::default();
    for _ in 0..6 * 120 {
        let events = tick(&mut s, l, true);
        tally.add(&events);
        impacts.collect(&s);
        for e in &events {
            if let Event::Fired { station, .. } = e
                && let Some(slot) = group(&s).slot(*station)
            {
                slots_fired[slot] = true;
            }
        }
    }
    let worst = impacts.worst(aim);
    println!(
        "fire-no-target: aim ({:.0}, {:.0}) shots={} ground_impacts={} distinct impact points={} worst {:.1} ft from the aim point (mean {:.1}); guns fired {:?}",
        aim[0],
        aim[2],
        tally.shots,
        tally.ground,
        impacts.points.len(),
        worst,
        impacts.mean(aim),
        slots_fired
    );
    report.check(
        tally.shots > 0,
        format!(
            "the trigger fired with nothing designated ({} rounds)",
            tally.shots
        ),
    );
    report.check(slots_fired == [true; 3], "all three linked guns fired");
    report.check(
        tally.ground > 0 && !impacts.points.is_empty(),
        format!(
            "rounds landed ({} ground impacts, {} distinct points)",
            tally.ground,
            impacts.points.len()
        ),
    );
    report.check(
        worst < 150.,
        format!("the impacts are around the aim point (worst {worst:.1} ft)"),
    );
    report.check(
        matches!(group(&s).sight, Sight::Free),
        "firing did not change the sight",
    );
    report.done("fire-no-target")
}

/// A ground object 5,000 feet abeam is tracked while the aircraft flies past
/// until it is behind the wing line: the guns stop at the arc, the status
/// reads CANNOT BEAR, the camera stays on the object and the trigger fires
/// (late, once the object is out of every arc, so the rounds do not kill it).
fn track_out_of_arc(config: &Configuration) -> AppResult<()> {
    let mut report = Report::default();
    let mut s = state(config)?;
    let speed = 300.;
    let at = |seconds: f64| launcher([0., 3000., speed * seconds], 0., 0., speed);
    let l0 = at(0.);
    s.range_target(0, l0);
    let object = [-5000., 0., 0.];
    {
        let t = &mut s.targets[0];
        t.position = object;
        t.velocity = [0.; 3];
        t.airborne = false;
        t.hp = 100_000;
        t.initial_hp = 100_000;
    }
    let id = s.targets[0].id;
    tick(&mut s, l0, false);
    group_mut(&mut s).look = gunship::body_angles(l0, offset(object, gunship::eye_position(l0)));
    s.command(0, Command::SightDesignate, l0);
    tick(&mut s, l0, false);
    report.check(
        group(&s).sight == Sight::Tracked(id),
        format!(
            "Backslash tracked the ground object ({:?})",
            group(&s).sight
        ),
    );
    // Inside the arc first: let the guns train and read READY.
    for t in 0..240 {
        tick(&mut s, at(f64::from(t) / HZ), false);
    }
    let candidate = group(&s).slot(s.own().selected).ok_or("no candidate gun")?;
    report.check(
        group(&s).status[candidate] == Readiness::Ready,
        format!(
            "abeam, the candidate gun reads READY ({:?})",
            group(&s).status[candidate]
        ),
    );
    // Fly past: 35 seconds is 10,500 feet, so the object ends 154 degrees
    // aft of the nose, past C_25's 150 degree limit (and C_40's and C_105's),
    // and still inside the guns' 13,000 feet.
    let mut tally = Tally::default();
    let mut seen_cannot_bear = [false; 3];
    let mut fired_out_of_arc = 0usize;
    let mut look_error = 0f64;
    for t in 240..35 * 120 {
        let l = at(f64::from(t) / HZ);
        let events = tick(&mut s, l, t >= 31 * 120);
        tally.add(&events);
        let g = group(&s);
        for slot in 0..3 {
            if g.status[slot] == Readiness::GunArc {
                seen_cannot_bear[slot] = true;
                fired_out_of_arc += events
                    .iter()
                    .filter(|e| matches!(e, Event::Fired { station, .. } if g.slot(*station) == Some(slot)))
                    .count();
            }
        }
        let want = gunship::body_angles(l, offset(object, gunship::eye_position(l)));
        let diff = |a: f64, b: f64| ((a - b + PI).rem_euclid(2. * PI) - PI).abs();
        look_error = look_error.max(diff(g.look[0], want[0]).max(diff(g.look[1], want[1])));
    }
    let g = group(&s);
    let aft = gunship::body_angles(at(35.), offset(object, gunship::eye_position(at(35.))));
    println!(
        "track-out-of-arc: object bearing at the end {:.1} deg, train headings {:?} deg, status {:?}, shots={} fired while CANNOT BEAR={}, worst camera error {:.4} deg",
        aft[0].to_degrees(),
        g.headings.map(f64::to_degrees),
        g.status,
        tally.shots,
        fired_out_of_arc,
        look_error.to_degrees()
    );
    for slot in 0..3 {
        if g.stations[slot].is_none() {
            continue;
        }
        let limit = gunship::HEADING_ARC[slot];
        let beyond = -FRAC_PI_2_F64 - limit;
        report.check(
            g.status[slot] == Readiness::GunArc,
            format!(
                "gun {slot} reads CANNOT BEAR at the end ({:?})",
                g.status[slot]
            ),
        );
        report.check(
            seen_cannot_bear[slot],
            format!("gun {slot} read CANNOT BEAR once the object passed its arc"),
        );
        report.check(
            (g.headings[slot] - beyond).abs() < 0.5_f64.to_radians(),
            format!(
                "gun {slot} stopped at its arc limit ({:.1} deg, limit {:.1})",
                g.headings[slot].to_degrees(),
                beyond.to_degrees()
            ),
        );
    }
    report.check(
        g.sight == Sight::Tracked(id),
        "the sight still tracks the object after it passed behind the wing",
    );
    report.check(
        look_error < 0.1_f64.to_radians(),
        format!(
            "the camera stayed on the object (worst error {:.4} deg)",
            look_error.to_degrees()
        ),
    );
    report.check(
        fired_out_of_arc > 0,
        format!("the trigger kept firing while CANNOT BEAR ({fired_out_of_arc} rounds)"),
    );
    report.done("track-out-of-arc")
}

const FRAC_PI_2_F64: f64 = PI / 2.;
