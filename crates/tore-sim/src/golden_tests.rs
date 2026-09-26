//! Behaviour fingerprints: the neutrality gate for the replays work.
//!
//! Added on 2026-09-26, before the replay milestones instrument this crate.
//! Those milestones add write-only "why" records: AI controller and actor
//! traces, a log of AI random draws, an AI message journal, a flight-model
//! trace and a per-shot outcome list. They also split two AI checks into
//! per-reason steps with the same arithmetic. None of that may change what
//! the simulation does. These tests fly synthetic AI missions, scripted
//! flight maneuvers and combat engagements, fold named behaviour fields into a
//! fingerprint every tick, and compare the result with the value recorded
//! here, so any change in behaviour fails loudly.
//!
//! How to read and maintain them:
//!
//! - Only named fields are hashed, never the `Debug` text of a struct, so a
//!   new trace field never moves a fingerprint. `Debug` text is used only for
//!   enums: a variant name and the plain values it carries.
//! - The fixtures are private copies, so editing another test's fixture never
//!   moves a fingerprint.
//! - Every scenario runs twice in one process and must match itself on every
//!   platform. The recorded values are compared only on macOS on Apple
//!   silicon, where they were generated, because maths library results can
//!   differ in the last bit between platforms.
//! - A deliberate behaviour change updates the recorded value in the same
//!   commit, and the commit message explains what changed and why. A
//!   toolchain or macOS update that only moves floating-point rounding is
//!   handled the same way, and the commit says so.
//! - `TORE_GOLDEN_VERBOSE=1` prints a fingerprint for every part of every
//!   scenario. Running it before and after a change shows where the first
//!   difference appears.

mod ai;
mod combat;
mod fixtures;
mod flight;

use std::cell::RefCell;
use std::fmt::{self, Write as _};

use tore_input::{PilotCommand, PilotInput};

/// Recorded fingerprints are compared only where they were generated.
const RECORDED_PLATFORM: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));

/// Order-sensitive 64-bit fingerprint. Each recorded value is folded in as one
/// 64-bit word through the SplitMix64 finalizer, which is a bijection, so a
/// change to any single recorded word always changes the result.
#[derive(Clone, Copy, Debug)]
struct Fingerprint(u64);

impl Default for Fingerprint {
    fn default() -> Self {
        Self(0x746f_7265_676f_6c64)
    }
}

impl Fingerprint {
    fn value(&self) -> u64 {
        self.0
    }

    fn word(&mut self, word: u64) {
        let mut z = (self.0 ^ word).wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        self.0 = z ^ (z >> 31);
    }

    fn f64(&mut self, value: f64) {
        self.word(value.to_bits());
    }

    fn vector(&mut self, value: [f64; 3]) {
        for axis in value {
            self.f64(axis);
        }
    }

    fn bool(&mut self, value: bool) {
        self.word(u64::from(value));
    }

    fn int(&mut self, value: impl Into<i64>) {
        self.word(value.into() as u64);
    }

    fn u64(&mut self, value: u64) {
        self.word(value);
    }

    /// A length or an index, recorded before the items it counts.
    fn count(&mut self, value: usize) {
        self.word(value as u64);
    }

    fn text(&mut self, text: &str) {
        self.count(text.len());
        let _ = self.write_str(text);
    }

    /// An enum's `Debug` text: its variant and any plain values it carries.
    /// Never used for a struct, which may gain fields.
    fn name(&mut self, value: &impl fmt::Debug) {
        let _ = write!(self, "{value:?}");
        self.word(u64::MAX);
    }

    fn option<T>(&mut self, value: Option<T>, record: impl FnOnce(&mut Self, T)) {
        match value {
            None => self.word(0),
            Some(value) => {
                self.word(1);
                record(self, value);
            }
        }
    }
}

impl fmt::Write for Fingerprint {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            self.word(u64::from(byte));
        }
        Ok(())
    }
}

/// Intermediate fingerprints of one run, so a failure can say where two runs
/// first differ and verbose mode can show where a change starts.
#[derive(Default)]
struct Probe {
    parts: RefCell<Vec<(String, u64)>>,
}

impl Probe {
    fn part(&self, label: impl Into<String>, value: u64) {
        self.parts.borrow_mut().push((label.into(), value));
    }
}

/// One scenario: its recorded fingerprint and two in-process runs.
struct Outcome {
    scenario: String,
    recorded: u64,
    first: u64,
    second: u64,
    first_divergence: Option<String>,
}

fn run_twice(scenario: &str, recorded: u64, run: impl Fn(&Probe) -> u64) -> Outcome {
    let (first_probe, second_probe) = (Probe::default(), Probe::default());
    let first = run(&first_probe);
    let second = run(&second_probe);
    let first_parts = first_probe.parts.into_inner();
    let second_parts = second_probe.parts.into_inner();
    if std::env::var_os("TORE_GOLDEN_VERBOSE").is_some() {
        for (label, value) in &first_parts {
            eprintln!("golden {scenario} {label} {value:#018x}");
        }
        eprintln!("golden {scenario} total {first:#018x}");
    }
    let first_divergence = first_parts
        .iter()
        .zip(&second_parts)
        .find(|(a, b)| a != b)
        .map(|(a, _)| a.0.clone());
    Outcome {
        scenario: scenario.into(),
        recorded,
        first,
        second,
        first_divergence,
    }
}

/// Fail with one line per broken scenario.
fn verify(outcomes: impl IntoIterator<Item = Outcome>) {
    let mut failures = Vec::new();
    for outcome in outcomes {
        if outcome.first != outcome.second {
            failures.push(format!(
                "{}: not deterministic, two identical runs gave {:#018x} and {:#018x} (first difference at {})",
                outcome.scenario,
                outcome.first,
                outcome.second,
                outcome.first_divergence.as_deref().unwrap_or("the end"),
            ));
        } else if RECORDED_PLATFORM && outcome.first != outcome.recorded {
            failures.push(format!(
                "{}: behaviour changed, recorded {:#018x}, now {:#018x}",
                outcome.scenario, outcome.recorded, outcome.first,
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "golden behaviour fingerprints failed:\n  {}\n\
         Recording and tracing code must never change these. If the behaviour change is \
         deliberate, update the recorded value for each scenario above (constants in \
         crates/tore-sim/src/golden_tests/) in the same commit and explain the behaviour change \
         in the commit message. To find where a change starts, run with TORE_GOLDEN_VERBOSE=1 \
         before and after it and compare the printed part fingerprints.",
        failures.join("\n  "),
    );
}

/// One tick of pilot input, commands included.
fn record_input(fp: &mut Fingerprint, input: &PilotInput) {
    fp.f64(input.pitch);
    fp.f64(input.roll);
    fp.f64(input.yaw);
    fp.f64(input.throttle_rate);
    fp.option(input.throttle, |fp, throttle| fp.f64(throttle));
    fp.count(input.commands.len());
    for command in &input.commands {
        match *command {
            PilotCommand::Eject => fp.u64(1),
            PilotCommand::Toggle(switch) => {
                fp.u64(2);
                fp.name(&switch);
            }
            PilotCommand::Set(switch, on) => {
                fp.u64(3);
                fp.name(&switch);
                fp.bool(on);
            }
            PilotCommand::Throttle(value) => {
                fp.u64(4);
                fp.f64(value);
            }
            PilotCommand::AdjustThrottle(value) => {
                fp.u64(5);
                fp.f64(value);
            }
        }
    }
}

/// One missile warning record, as an aircraft's threat service holds it.
fn record_threat(fp: &mut Fingerprint, record: &crate::combat::threats::ThreatRecord) {
    fp.int(record.missile_id);
    fp.name(&record.source);
    fp.u64(record.observed_tick);
    fp.f64(record.bearing_deg);
    fp.option(record.position, |fp, position| fp.vector(position));
    fp.option(record.velocity, |fp, velocity| fp.vector(velocity));
    fp.option(record.guidance_class, |fp, class| fp.name(&class));
    fp.bool(record.targeting_receiver);
    fp.bool(record.was_targeting_receiver);
    fp.bool(record.stale);
    fp.option(record.radar_bearing_deg, |fp, bearing| fp.f64(bearing));
}

/// Every behaviour field of one aircraft's flight state. Presentation and
/// configuration are left out; the private lift memory shows up through the
/// state it drives.
fn record_flight(fp: &mut Fingerprint, s: &crate::flight::State) {
    fp.vector(s.position);
    fp.f64(s.yaw);
    fp.f64(s.pitch);
    fp.f64(s.bank);
    fp.f64(s.speed);
    fp.vector(s.velocity);
    fp.f64(s.roll_rate);
    fp.f64(s.pitch_rate);
    fp.vector(s.auxiliary_rates);
    fp.f64(s.vertical_speed);
    fp.f64(s.g);
    let m = &s.maneuver;
    fp.u64(m.tick);
    fp.f64(m.commanded_g);
    fp.f64(m.lift_g);
    fp.f64(m.achieved_g);
    fp.vector(m.body_rates_rad_per_second);
    fp.f64(m.rudder_command);
    fp.f64(m.rudder_deflection);
    fp.f64(m.effective_rudder);
    fp.option(m.departure, |fp, mode| fp.name(&mode));
    fp.int(m.stall_severity_f8);
    fp.f64(s.throttle);
    fp.f64(s.fuel);
    fp.f64(s.payload_lbs);
    fp.bool(s.engine);
    fp.bool(s.burner);
    fp.f64(s.exhaust);
    fp.f64(s.rudder);
    fp.f64(s.elevator);
    fp.f64(s.aileron);
    for device in [s.gear, s.flaps, s.brake, s.hook, s.bay] {
        fp.f64(device);
    }
    for switch in [
        s.bay_open,
        s.bay_auto_open,
        s.gear_down,
        s.flaps_down,
        s.brake_out,
        s.hook_down,
        s.radar,
        s.jammer,
    ] {
        fp.bool(switch);
    }
    fp.f64(s.damage_fraction);
    fp.option(s.damage_variant, |fp, variant| fp.count(variant));
    for region in s.damage_regions {
        fp.f64(region);
    }
    fp.name(&s.autopilot.mode());
    fp.option(s.escape.as_ref(), |fp, escape| {
        fp.vector(escape.position);
        fp.vector(escape.velocity);
        fp.f64(escape.heading);
        fp.name(&escape.phase);
        fp.u64(escape.ticks);
    });
    fp.option(s.eject_armed_at, |fp, tick| fp.u64(tick));
    fp.bool(s.crashed);
    fp.option(s.wreck.as_ref(), |fp, wreck| {
        fp.name(&wreck.phase);
        fp.u64(wreck.ticks);
        fp.int(wreck.polls);
        fp.vector(wreck.angular_rates);
        for share in wreck.power.acceleration {
            fp.f64(share);
        }
        fp.int(wreck.power.engine_count);
        fp.f64(wreck.power.fuel_seconds);
    });
    fp.u64(s.ticks);
    fp.vector(s.jolt);
    fp.option(s.research.as_ref(), |fp, r| {
        fp.name(&r.departure.mode);
        fp.int(r.departure.elapsed);
        fp.int(r.spinning);
        fp.f64(r.spin_rate);
        fp.bool(r.on_ground);
        fp.int(r.severity_f8);
        fp.bool(r.stall_active);
        fp.int(r.elapsed);
        fp.int(r.landings.count);
        fp.int(r.landings.score);
        // The next draw stands in for the generator's private state.
        fp.int(r.rng.clone().below(i32::MAX).unwrap_or(-1));
    });
    fp.option(s.native.as_ref(), |fp, n| {
        fp.int(n.elapsed);
        fp.option(n.fault.as_deref(), |fp, fault| fp.text(fault));
        fp.int(n.rng.clone().below(i32::MAX).unwrap_or(-1));
    });
    let systems = &s.systems;
    for count in systems.counts {
        fp.int(count);
    }
    fp.f64(systems.engine.temperature);
    fp.f64(systems.engine.power);
    fp.f64(systems.engine.flameout);
    fp.f64(systems.fluids.oil);
    fp.f64(systems.fluids.hydraulic);
    for tank in systems.fuel.external {
        fp.f64(tank);
    }
    fp.option(systems.controls.throttle_lock, |fp, lock| fp.f64(lock));
    fp.bool(systems.structure.failed);
    fp.bool(systems.structure.wing_damage);
    fp.bool(systems.structure.burning());
    fp.bool(systems.pilot.dead);
    fp.bool(systems.pilot.ejected);
    fp.count(systems.messages.len());
    for message in &systems.messages {
        fp.text(message);
    }
}
