//! Impossible-state checks for the headless AI probe (`--ai-probe-ticks`).
//!
//! A test harness only (agent decision, 2026-09-28, overnight bug battery):
//! it reads the mission after every tick and never writes to it, so the probe's
//! other output and its checksum are unchanged. Each kind of problem is printed
//! once per aircraft as an `AI probe anomaly:` line, and a closing
//! `AI probe invariants:` line counts them, for `tools/battery_scenarios/ai.py`.
//!
//! The limits are deliberately loose: they catch states no aircraft can be in
//! (alive at zero hit points, under the ground, firing after death, ammunition
//! appearing), not flying the spec would call poor.

use std::collections::{BTreeMap, BTreeSet};

use tore_sim::wreck::Phase;

use crate::{ai_wings, combat, terrain};

/// An alive aircraft this far below the surface is under the ground, feet.
const UNDER_GROUND_FT: f64 = 20.;
/// Above this height above the surface an aircraft is airborne, feet.
const AIRBORNE_AGL_FT: f64 = 50.;
/// Faster than this is not an aircraft speed, feet per second (about 2,400 kt).
const ABSURD_SPEED_FPS: f64 = 4000.;
/// Higher than this is not an aircraft altitude, feet.
const ABSURD_ALTITUDE_FT: f64 = 120_000.;
/// An airborne alive aircraft slower than this, feet per second (about 30 kt),
/// for [`SLOW_TICKS`] is hanging in the air.
const SLOW_FPS: f64 = 50.;
const SLOW_TICKS: u64 = 10 * 120;
/// An airborne alive aircraft that moves less than this in [`FROZEN_TICKS`] is frozen.
const FROZEN_FT: f64 = 1.;
const FROZEN_TICKS: u64 = 2 * 120;
/// Heading rate that, held for [`SPIN_TICKS`], means an aircraft is spinning
/// rather than turning, degrees per second.
const SPIN_DEG_S: f64 = 40.;
const SPIN_TICKS: u64 = 30 * 120;
/// Two aircraft centres closer than this a tick before one dies were touching
/// (two 28 ft contact radii plus one tick of closure at 300 ft/s), feet.
const TOUCHING_FT: f64 = 60.;
/// Within [`FLAP_TICKS`], this many activity changes or pitch-stick reversals
/// mean the AI is flipping between two decisions every few ticks for seconds.
/// A second of dithering at a weapon-envelope edge stays below these.
const FLAP_TICKS: u64 = 360;
const FLAP_ACTIVITY_CHANGES: usize = 30;
const FLAP_PITCH_REVERSALS: usize = 60;
/// An aircraft in an airfield phase below this height above the surface is
/// on its wheels for the pavement check, feet.
const GROUND_AGL_FT: f64 = 20.;
/// The airfield phases spent on the wheels: the pavement check counts an
/// aircraft in one of them at a redrawn airport off the drawn pavement as an
/// anomaly (slice CL-AP, John 2026-10-11: AI taxi on taxiways, not grass).
const GROUND_PHASES: [tore_sim::ai::airfield::Phase; 7] = {
    use tore_sim::ai::airfield::Phase;
    [
        Phase::Waiting,
        Phase::Taxi,
        Phase::LineUp,
        Phase::TakeoffRoll,
        Phase::Rollout,
        Phase::TaxiClear,
        Phase::Parked,
    ]
};

#[derive(Default)]
struct Track {
    /// The combat wreck's position and phase last tick.
    last_wreck: Option<([f64; 3], Phase)>,
    frozen_since: Option<(u64, [f64; 3])>,
    slow_since: Option<u64>,
    spin_since: Option<u64>,
    last_yaw: Option<f64>,
    last_bank: Option<f64>,
    zero_hp_ticks: u32,
    rounds: Option<u32>,
    decoys: Option<u32>,
    death_tick: Option<u64>,
    peak_heading_rate: f64,
    peak_bank_rate: f64,
    peak_speed: f64,
    peak_altitude: f64,
    last_activity: Option<tore_sim::ai::controller::Activity>,
    /// Ticks of recent activity changes and pitch-stick reversals.
    activity_changes: std::collections::VecDeque<u64>,
    pitch_sign: f64,
    pitch_reversals: std::collections::VecDeque<u64>,
}

#[derive(Default)]
pub struct ProbeInvariants {
    tracks: BTreeMap<u32, Track>,
    reported: BTreeSet<(u32, &'static str)>,
    /// Alive aircraft positions after the previous tick.
    previous_positions: BTreeMap<u32, [f64; 3]>,
    seen_projectiles: BTreeSet<u32>,
    anomalies: u32,
    samples: u64,
    kinds: BTreeMap<&'static str, u32>,
    /// Print the closing `AI probe pavement:` line (`--probe-pavement`).
    pub report_pavement: bool,
    /// Ticks an AI aircraft spent on the ground in an airfield phase at a
    /// redrawn airport, and how many of them off its drawn pavement.
    pavement_samples: u64,
    off_pavement: u64,
    /// `--probe-track FILE`: every sixth ground tick of each AI aircraft at
    /// a redrawn airport, as `tick label phase leg x z on|off` lines, for
    /// `--airfield-audit --track` to draw.
    pub track: Option<std::io::BufWriter<std::fs::File>>,
}

impl ProbeInvariants {
    fn flag(&mut self, tick: u64, id: u32, label: &str, kind: &'static str, detail: String) {
        *self.kinds.entry(kind).or_default() += 1;
        if self.reported.insert((id, kind)) {
            self.anomalies += 1;
            println!(
                "AI probe anomaly: t={tick} ({:.1}s) {kind}: {label} {detail}",
                tick as f64 / 120.
            );
        }
    }

    /// Check every AI aircraft after a whole probe tick.
    pub fn observe(
        &mut self,
        tick: u64,
        bridge: &ai_wings::AiWings,
        combat: &combat::Combat,
        world: &terrain::Terrain,
    ) {
        let cell = f64::from(tore_formats::theater::CELL_FEET);
        let extent = [
            world.theater.cols.saturating_sub(1) as f64 * cell,
            world.theater.rows.saturating_sub(1) as f64 * cell,
        ];
        self.samples += 1;
        let state = &combat.state;
        for slot in bridge.slots() {
            let Some(actor) = bridge.mission().actor(slot.id) else {
                continue;
            };
            let id = slot.id;
            let label = slot.label();
            let f = actor.flight();
            let target = state.targets.iter().find(|t| t.id == id);
            let [x, y, z] = f.position;
            let finite = f.position.iter().chain(&f.velocity).all(|v| v.is_finite())
                && f.speed.is_finite()
                && f.yaw.is_finite()
                && f.bank.is_finite()
                && f.pitch.is_finite();
            if !finite {
                self.flag(
                    tick,
                    id,
                    &label,
                    "non-finite motion",
                    format!("{:?}", f.position),
                );
                continue;
            }
            let ground = world.surface(x, z).height;
            let agl = y - ground;
            let alive = actor.alive();
            let hp = target.map(|t| t.hp);
            let track = self.tracks.entry(id).or_default();
            let rounds = actor.rounds_remaining();
            let decoys: u32 = actor.dispensers().iter().map(|d| d.count).sum();
            let previous_rounds = track.rounds.replace(rounds);
            let previous_decoys = track.decoys.replace(decoys);
            let turn = |a: f64, b: f64| {
                (a - b + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                    - std::f64::consts::PI
            };
            let heading_rate = track
                .last_yaw
                .replace(f.yaw)
                .map_or(0., |p| turn(f.yaw, p).abs().to_degrees() * 120.);
            let bank_rate = track
                .last_bank
                .replace(f.bank)
                .map_or(0., |p| turn(f.bank, p).abs().to_degrees() * 120.);
            if !alive && track.death_tick.is_none() {
                track.death_tick = Some(tick);
            }
            let death_tick = track.death_tick;
            let airborne = alive && !f.crashed && agl > AIRBORNE_AGL_FT;
            let mut flags: Vec<(&'static str, String)> = Vec::new();
            if alive {
                track.peak_speed = track.peak_speed.max(f.speed);
                track.peak_altitude = track.peak_altitude.max(y);
                if airborne {
                    track.peak_heading_rate = track.peak_heading_rate.max(heading_rate);
                    track.peak_bank_rate = track.peak_bank_rate.max(bank_rate);
                }
                if hp.is_some_and(|hp| hp <= 0) {
                    track.zero_hp_ticks += 1;
                    // One tick of grace: the bridge learns of a kill next tick.
                    if track.zero_hp_ticks > 2 {
                        flags.push(("alive at zero hit points", format!("hp={hp:?}")));
                    }
                } else {
                    track.zero_hp_ticks = 0;
                }
                if !f.crashed && agl < -UNDER_GROUND_FT {
                    flags.push(("alive under the ground", format!("agl={agl:.0} y={y:.0}")));
                }
                if !(0.0..=extent[0]).contains(&x) || !(0.0..=extent[1]).contains(&z) {
                    flags.push(("outside the world", format!("x={x:.0} z={z:.0}")));
                }
                if f.speed > ABSURD_SPEED_FPS {
                    flags.push(("absurd speed", format!("kt={:.0}", f.speed / 1.68781)));
                }
                if y > ABSURD_ALTITUDE_FT {
                    flags.push(("absurd altitude", format!("y={y:.0}")));
                }
                if matches!(
                    actor.activity(),
                    tore_sim::ai::controller::Activity::Destroyed
                ) {
                    flags.push(("alive but labelled Destroyed", String::new()));
                }
            } else {
                // The label follows on the actor's next decision tick.
                if death_tick.is_some_and(|d| tick > d + 2)
                    && !matches!(
                        actor.activity(),
                        tore_sim::ai::controller::Activity::Destroyed
                    )
                {
                    flags.push((
                        "dead but not labelled Destroyed",
                        format!("activity={}", actor.activity().label()),
                    ));
                }
                // A falling wreck moves; a grounded or burst one must not.
                let now = target.and_then(|t| Some((t.position, t.wreck.as_ref()?.phase)));
                if let (Some((before, was)), Some((at, _))) = (track.last_wreck, now)
                    && was != Phase::Falling
                {
                    let moved = (0..3)
                        .map(|i| (before[i] - at[i]).powi(2))
                        .sum::<f64>()
                        .sqrt();
                    if moved > 0.01 {
                        flags.push((
                            "settled wreck moving",
                            format!("moved={moved:.2} phase={was:?}"),
                        ));
                    }
                }
                track.last_wreck = now;
                if death_tick.is_some_and(|d| tick > d + 1)
                    && previous_rounds.is_some_and(|r| rounds < r)
                {
                    flags.push(("dead aircraft spent ammunition", format!("rounds={rounds}")));
                }
                if death_tick.is_some_and(|d| tick > d + 1)
                    && previous_decoys.is_some_and(|d| decoys < d)
                {
                    flags.push(("dead aircraft released decoys", format!("decoys={decoys}")));
                }
            }
            if previous_rounds.is_some_and(|r| rounds > r) {
                flags.push((
                    "ammunition increased",
                    format!("{} -> {rounds}", previous_rounds.unwrap_or(0)),
                ));
            }
            if previous_decoys.is_some_and(|d| decoys > d) {
                flags.push((
                    "decoys increased",
                    format!("{} -> {decoys}", previous_decoys.unwrap_or(0)),
                ));
            }
            // A helicopter, the V-22 or a hovering jet in a vertical airfield
            // phase hovers and creeps on purpose (slice H2).
            let hovering = actor.airfield().is_some_and(|s| {
                use tore_sim::ai::airfield::Phase as P;
                matches!(
                    s.phase(),
                    P::VerticalHold
                        | P::VerticalClimb
                        | P::Transition
                        | P::HoverApproach
                        | P::VerticalDescent
                        | P::Settle
                        | P::AirTaxi
                )
            });
            // A helicopter or the V-22 flies slowly on purpose; hanging in
            // the air is a fixed-wing aircraft's stall.
            let rotorcraft = tore_sim::ai::rotor_adapter::flies(f);
            if airborne && f.speed < SLOW_FPS && !hovering && !rotorcraft {
                let since = *track.slow_since.get_or_insert(tick);
                if tick - since >= SLOW_TICKS {
                    flags.push((
                        "hanging in the air",
                        format!("kt={:.0} agl={agl:.0}", f.speed / 1.68781),
                    ));
                }
            } else {
                track.slow_since = None;
            }
            if airborne && !hovering {
                match track.frozen_since {
                    Some((since, at))
                        if (0..3)
                            .map(|i| (at[i] - f.position[i]).powi(2))
                            .sum::<f64>()
                            .sqrt()
                            < FROZEN_FT =>
                    {
                        if tick - since >= FROZEN_TICKS {
                            flags.push(("frozen in the air", format!("agl={agl:.0}")));
                        }
                    }
                    _ => track.frozen_since = Some((tick, f.position)),
                }
                if heading_rate > SPIN_DEG_S {
                    let since = *track.spin_since.get_or_insert(tick);
                    if tick - since >= SPIN_TICKS {
                        flags.push(("spinning", format!("heading_rate={heading_rate:.0}")));
                    }
                } else {
                    track.spin_since = None;
                }
            } else {
                track.frozen_since = None;
                track.spin_since = None;
            }
            if alive {
                let activity = actor.activity();
                if track
                    .last_activity
                    .replace(activity)
                    .is_some_and(|a| a != activity)
                {
                    track.activity_changes.push_back(tick);
                }
                let pitch = actor.last_input().pitch;
                if pitch.abs() > 0.05 {
                    if pitch.signum() == -track.pitch_sign {
                        track.pitch_reversals.push_back(tick);
                    }
                    track.pitch_sign = pitch.signum();
                }
                for queue in [&mut track.activity_changes, &mut track.pitch_reversals] {
                    while queue.front().is_some_and(|t| tick - t >= FLAP_TICKS) {
                        queue.pop_front();
                    }
                }
                if track.activity_changes.len() >= FLAP_ACTIVITY_CHANGES {
                    flags.push((
                        "activity flapping",
                        format!(
                            "{} changes in 3 s, now {}",
                            track.activity_changes.len(),
                            activity.label()
                        ),
                    ));
                }
                if track.pitch_reversals.len() >= FLAP_PITCH_REVERSALS {
                    flags.push((
                        "pitch stick oscillating",
                        format!("{} reversals in 3 s", track.pitch_reversals.len()),
                    ));
                }
            }
            if alive
                && !f.crashed
                && agl < GROUND_AGL_FT
                && let Some(sequence) = actor.airfield()
                && GROUND_PHASES.contains(&sequence.phase())
                && tore_world::terrain::redrawn::Built::for_runway(
                    &world.redrawn,
                    sequence.runway().object,
                )
                .is_some()
            {
                self.pavement_samples += 1;
                let on = tore_world::terrain::redrawn::on_pavement(&world.redrawn, x, z);
                if let Some(out) = &mut self.track
                    && tick.is_multiple_of(6)
                {
                    use std::io::Write;
                    let _ = writeln!(
                        out,
                        "{tick} {} {:?} {} {x:.1} {z:.1} {} hdg={:.1} track={:.1} fps={:.1}",
                        label.replace(' ', "_"),
                        sequence.phase(),
                        sequence.leg(),
                        if on { "on" } else { "off" },
                        f.yaw.to_degrees().rem_euclid(360.),
                        f.velocity[0]
                            .atan2(f.velocity[2])
                            .to_degrees()
                            .rem_euclid(360.),
                        f.velocity[0].hypot(f.velocity[2]),
                    );
                }
                if !on {
                    self.off_pavement += 1;
                    flags.push((
                        "off the pavement",
                        format!(
                            "phase={:?} leg={} x={x:.0} z={z:.0}",
                            sequence.phase(),
                            sequence.leg()
                        ),
                    ));
                }
            }
            for (kind, detail) in flags {
                self.flag(tick, id, &label, kind, detail);
            }
        }
        // Aircraft that died this tick within touching distance of another
        // aircraft (as they were last tick) collided in the air.
        let now: Vec<_> = bridge
            .slots()
            .iter()
            .filter_map(|slot| {
                let actor = bridge.mission().actor(slot.id)?;
                Some((
                    slot.id,
                    slot.label(),
                    actor.flight().position,
                    actor.alive(),
                ))
            })
            .collect();
        let previous = std::mem::take(&mut self.previous_positions);
        for (id, label, _, alive) in &now {
            if *alive || self.tracks.get(id).and_then(|t| t.death_tick) != Some(tick) {
                continue;
            }
            let Some(at) = previous.get(id) else { continue };
            for (other, other_label, other_at) in previous
                .iter()
                .filter(|(other, _)| *other != id)
                .filter_map(|(other, p)| {
                    now.iter()
                        .find(|n| n.0 == *other)
                        .map(|n| (*other, n.1.clone(), *p))
                })
            {
                let gap = (0..3)
                    .map(|k| (at[k] - other_at[k]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                if gap < TOUCHING_FT {
                    self.flag(
                        tick,
                        *id,
                        label,
                        "mid-air collision",
                        format!("with {other_label} (id {other}) {gap:.0} ft apart a tick earlier"),
                    );
                }
            }
        }
        self.previous_positions = now.iter().filter(|n| n.3).map(|n| (n.0, n.2)).collect();
        // Rounds and missiles that appeared this tick, from a dead owner.
        let mut fresh = Vec::new();
        for p in &state.projectiles {
            if self.seen_projectiles.insert(p.id) && p.owner != 0 {
                fresh.push(p.owner);
            }
        }
        for owner in fresh {
            let dead_long = self
                .tracks
                .get(&owner)
                .and_then(|t| t.death_tick)
                .is_some_and(|d| tick > d + 1);
            if dead_long {
                let label = bridge
                    .slot(owner)
                    .map_or_else(|| format!("id {owner}"), |s| s.label());
                self.flag(tick, owner, &label, "dead aircraft fired", String::new());
            }
        }
    }

    pub fn summary(&self, bridge: &ai_wings::AiWings) {
        let mut heading: f64 = 0.;
        let mut bank: f64 = 0.;
        let mut speed: f64 = 0.;
        let mut altitude: f64 = 0.;
        for track in self.tracks.values() {
            heading = heading.max(track.peak_heading_rate);
            bank = bank.max(track.peak_bank_rate);
            speed = speed.max(track.peak_speed);
            altitude = altitude.max(track.peak_altitude);
        }
        let kinds: Vec<_> = self.kinds.iter().map(|(k, n)| format!("{k}={n}")).collect();
        println!(
            "AI probe invariants: actors={} samples={} anomalies={} peak_heading_rate={heading:.1} peak_bank_rate={bank:.1} peak_kt={:.0} peak_alt_ft={altitude:.0} kinds=[{}]",
            bridge.slots().len(),
            self.samples,
            self.anomalies,
            speed / 1.68781,
            kinds.join(", ")
        );
        if self.report_pavement {
            println!(
                "AI probe pavement: ground_samples={} off_pavement={}",
                self.pavement_samples, self.off_pavement
            );
        }
    }
}
