//! The bot's air-to-ground pilot (protocol 23, slice NA; `tore-bot --a2g`):
//! a laser designator and a bomber that drops on its spot, for the net
//! lane's `net-a2g-loopback`. Everything it does goes through the seat's
//! own commands and trigger, and everything it reads is what its screen
//! has: the readout the host built for it and the picture.
//!
//! - **Lase** (an aircraft with a designator, the F-16C's Pave Spike): turns
//!   the A-G radar on, designates the nearest enemy ground object (Ctrl+X,
//!   which needs the Easy targeting cheat), which the sight then tracks, and
//!   turns the laser on by hand (Ctrl+L). It flies at the object and circles
//!   it at about two and a half miles.
//! - **Buddy** (an aircraft with a laser bomb and an anti-radiation missile
//!   on its stations, no designation of its own): selects the laser bomb,
//!   flies at the buddy's spot, releases when the guided sight says IN RNG
//!   on that spot, follows the bomb to its end, then selects the missile,
//!   turns at the spot and fires in BORE at the HARM cue's candidate once the
//!   seeker holds it.
//!
//! - **Strike** (slice ST-K, `net-ai-strike-handback`): a bomber that sets up
//!   a level pass and is then dropped: turns the A-G radar on, designates the
//!   nearest enemy ground object (Ctrl+X), selects its Mk 82s, puts the bomb
//!   sight in LVL (Ctrl+M) and flies level at the target, never pressing the
//!   trigger. It says once what it set (`strike set ...`).
//! - **Watch**: presses nothing; says what its cockpit holds every two
//!   seconds (`cockpit ...`: the designation, station, bomb sight mode, laser
//!   hand setting and rounds), and reports other planes' weapons. A rejoined
//!   game flies it to show what it found.
//!
//! All print what their screens show (`a2g: ...` lines): their own or the
//! buddy's spot, the weapons they release and their ends, the HARM cue, and
//! each other's weapons in their picture, so a test can check that the
//! screens agree.

use super::{Controls, ScriptedPilot, length, sub};
use crate::client::ClientFrame;
use std::collections::BTreeSet;
use std::time::Duration;
use tore_sim::attitude::Vector;
use tore_sim::combat::delivery::DeliveryMode;
use tore_sim::combat::laser::LaserState;
use tore_sim::combat::live::{Command, Readiness};
use tore_sim::flight;
use tore_world::readout::{CockpitReadout, SpotReadout};
use tore_world::seats::SeatCommand;

/// What the bot does with its air-to-ground stores.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Designate a ground object and lase it.
    Lase,
    /// Drop a laser bomb on another plane's spot, then fire a HARM in BORE.
    Buddy,
    /// Set up a level bomb pass at the nearest enemy ground object and fly
    /// it without releasing (slice ST-K).
    Strike,
    /// Select the Mk 82s and drop one in CCIP every four seconds, wherever
    /// it is (slice BP3, `net-a2g-stores`: other games see the stores go).
    Bomb,
    /// Press nothing; report the cockpit (slice ST-K).
    Watch,
}

impl Role {
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "lase" => Some(Self::Lase),
            "buddy" => Some(Self::Buddy),
            "strike" => Some(Self::Strike),
            "bomb" => Some(Self::Bomb),
            "watch" => Some(Self::Watch),
            _ => None,
        }
    }
}

/// The laser bomb the buddy drops and the missile it fires.
const BOMB: &str = "GBU10.JT";
const MISSILE: &str = "AGM88.JT";
/// The strike role's bomb.
const STRIKE_BOMB: &str = "MK82.JT";
/// The lasing bot circles its target this far out, feet.
const ORBIT_FT: f64 = 15_000.;
/// How often the bot prints what it sees.
const EVERY: Duration = Duration::from_secs(2);
/// How long a press of the trigger is held.
const PRESS: Duration = Duration::from_millis(250);
/// The buddy waits this long after the bomb's end before it turns to the
/// HARM, so the lines come in order.
const AFTER_BOMB: Duration = Duration::from_secs(2);
/// After the bomb the buddy flies away from the target this long before it
/// turns back at it with the HARM.
const OUTBOUND: Duration = Duration::from_secs(25);
/// Closer than this to the point, feet, the bot holds its heading rather
/// than steer at a point it is passing over.
const OVERHEAD_FT: f64 = 6_000.;

/// The air-to-ground script of one bot. See the module notes.
#[derive(Clone, Debug)]
pub struct Script {
    role: Role,
    seated: Option<Duration>,
    /// Commands given once: the A-G radar, the designation, the laser.
    radar_at: Option<Duration>,
    designate_at: Option<Duration>,
    laser_at: Option<Duration>,
    /// The last time a weapon was stepped to, and the last line printed.
    stepped_at: Option<Duration>,
    printed_at: Option<Duration>,
    /// The trigger is held until then.
    held_until: Option<Duration>,
    /// The bomb was released, and when its end was seen.
    released: bool,
    bomb_ended: Option<Duration>,
    impact_tick: Option<u64>,
    /// The HARM was fired.
    fired: bool,
    /// The last HARM cue line, for its changes.
    cue_line: Option<String>,
    /// Where the spot last was: what the bot flies at.
    point: Option<Vector>,
    /// The buddy has seen a spot; before that it follows the nearest
    /// friendly aircraft.
    spotted: bool,
    /// The buddy flies away from the point until then, after its bomb.
    outbound_until: Option<Duration>,
    /// The projectiles of other planes already reported.
    seen: BTreeSet<u32>,
    /// The strike role said what it set; its last Ctrl+M.
    announced: bool,
    mode_at: Option<Duration>,
    /// Lines for `tore-bot` to print, in order.
    pub lines: Vec<String>,
}

/// The laser hand setting (Ctrl+L) in words.
fn hand_words(manual: Option<bool>) -> &'static str {
    match manual {
        Some(true) => "hand-on",
        Some(false) => "hand-off",
        None => "auto",
    }
}

fn at(p: Vector) -> String {
    format!("{:.0} {:.0} {:.0}", p[0], p[1], p[2])
}

fn spot_words(spot: &SpotReadout) -> String {
    format!(
        "owner {} target {} at {} range {:.0}",
        spot.spot.owner,
        spot.spot
            .target
            .map_or_else(|| "none".into(), |t| format!("{t:#x}")),
        at(spot.spot.position),
        spot.range_ft
    )
}

impl Script {
    pub fn new(role: Role) -> Self {
        Self {
            role,
            seated: None,
            radar_at: None,
            designate_at: None,
            laser_at: None,
            stepped_at: None,
            printed_at: None,
            held_until: None,
            released: false,
            bomb_ended: None,
            impact_tick: None,
            fired: false,
            cue_line: None,
            point: None,
            spotted: false,
            outbound_until: None,
            seen: BTreeSet::new(),
            announced: false,
            mode_at: None,
            lines: Vec::new(),
        }
    }

    pub fn role(&self) -> Role {
        self.role
    }

    fn say(&mut self, line: String) {
        self.lines.push(format!("a2g: {line}"));
    }

    /// The controls at `now`: the pilot's steering at the target and the
    /// script's commands and trigger. `frame` is the newest frame drawn.
    pub fn controls(
        &mut self,
        now: Duration,
        flight: &flight::State,
        frame: Option<&ClientFrame>,
        enemies: &BTreeSet<u32>,
        pilot: &mut ScriptedPilot,
    ) -> Controls {
        let seated = *self.seated.get_or_insert(now);
        let since = now.saturating_sub(seated);
        let mut controls = Controls::default();
        if let Some(frame) = frame {
            self.see(frame);
            if let Some(readout) = &frame.readout {
                match self.role {
                    Role::Lase => self.lase(now, since, readout, &mut controls),
                    Role::Buddy => self.buddy(now, readout, frame, enemies, &mut controls),
                    Role::Strike | Role::Bomb => {
                        self.strike(now, since, readout, frame, &mut controls)
                    }
                    Role::Watch => {}
                }
                if self
                    .printed_at
                    .is_none_or(|t| now.saturating_sub(t) >= EVERY)
                {
                    self.printed_at = Some(now);
                    self.report(readout);
                }
            }
        }
        let outbound = self.outbound_until.is_some_and(|until| now < until);
        controls.pilot = self.steer(flight, pilot, outbound).pilot;
        controls.trigger = self.held_until.is_some_and(|until| now < until);
        controls
    }

    /// The lasing bot's commands: A-G, Ctrl+X, then Ctrl+L once the sight
    /// tracks what was designated.
    fn lase(
        &mut self,
        now: Duration,
        since: Duration,
        readout: &CockpitReadout,
        controls: &mut Controls,
    ) {
        if self.radar_at.is_none() && since >= Duration::from_secs(2) {
            self.radar_at = Some(now);
            controls
                .commands
                .push(SeatCommand::Combat(Command::ToggleAirGround));
            self.say("A-G radar on".into());
        }
        if self.radar_at.is_some()
            && readout.targets.designated.is_none()
            && self
                .designate_at
                .is_none_or(|t| now.saturating_sub(t) >= Duration::from_secs(2))
            && since >= Duration::from_secs(4)
        {
            self.designate_at = Some(now);
            controls
                .commands
                .push(SeatCommand::Combat(Command::NearestGround));
        }
        let tracked = readout.sight.and_then(|s| s.mode.tracked());
        if let (Some(target), None) = (tracked, self.laser_at) {
            self.laser_at = Some(now);
            controls
                .commands
                .push(SeatCommand::Combat(Command::ToggleLaser));
            self.say(format!("tracking {target:#x}, laser on"));
        }
        self.point = readout
            .laser
            .and_then(|l| l.spot.filter(|s| s.own))
            .map(|s| s.spot.position)
            .or_else(|| readout.sight.filter(|s| s.mode.holds()).and_then(|s| s.aim))
            .or(self.point);
    }

    /// The buddy's commands: the bomb selected, released on IN RNG at the
    /// buddy's spot; its end; then the HARM in BORE.
    fn buddy(
        &mut self,
        now: Duration,
        readout: &CockpitReadout,
        frame: &ClientFrame,
        enemies: &BTreeSet<u32>,
        controls: &mut Controls,
    ) {
        if let Some(spot) = readout.laser.and_then(|l| l.spot.filter(|s| !s.own)) {
            self.point = Some(spot.spot.position);
            self.spotted = true;
        } else if !self.spotted {
            // No spot yet: follow the nearest friendly aircraft, the lasing
            // plane, which flies at its target.
            let own = frame.flight.position;
            self.point = frame
                .picture
                .targets
                .iter()
                .filter(|t| {
                    t.aircraft.is_some() && t.id != frame.plane.0 && !enemies.contains(&t.id)
                })
                .min_by(|a, b| {
                    length(sub(a.position, own)).total_cmp(&length(sub(b.position, own)))
                })
                .map(|t| t.position)
                .or(self.point);
        }
        let selected = frame
            .config
            .stations
            .get(usize::from(readout.stores.selected))
            .map(|s| s.weapon.source.as_str());
        let wanted = if self
            .bomb_ended
            .is_some_and(|t| now.saturating_sub(t) >= AFTER_BOMB)
        {
            MISSILE
        } else {
            BOMB
        };
        if selected != Some(wanted) && !(wanted == BOMB && self.released) {
            if self
                .stepped_at
                .is_none_or(|t| now.saturating_sub(t) >= Duration::from_millis(500))
            {
                self.stepped_at = Some(now);
                controls
                    .commands
                    .push(SeatCommand::CycleWeapon { forward: true });
            }
            return;
        }
        if wanted == BOMB && !self.released {
            if let Some(g) = readout.guided
                && g.in_reach
                && g.buddy
            {
                self.released = true;
                self.held_until = Some(now + PRESS);
                let spot = readout.laser.and_then(|l| l.spot);
                self.say(format!(
                    "release {BOMB} buddy yes reach {} range {} spot {}",
                    g.reach.map_or_else(
                        || "none".into(),
                        |r| format!("{:.0}-{:.0}", r.minimum_ft, r.maximum_ft)
                    ),
                    g.ground_range_ft
                        .map_or_else(|| "none".into(), |r| format!("{r:.0}")),
                    spot.as_ref().map_or_else(|| "none".into(), spot_words)
                ));
            }
            return;
        }
        if self.released && self.bomb_ended.is_none() {
            if let Some(tick) = readout.guided_impact_tick
                && self.impact_tick != Some(tick)
                && readout.weapon_tti.is_empty()
            {
                self.impact_tick = Some(tick);
                self.bomb_ended = Some(now);
                self.say(format!("bomb ended at tick {tick}"));
            }
            return;
        }
        if wanted == MISSILE && !self.fired {
            self.outbound_until.get_or_insert(now + OUTBOUND);
            let cue = readout.estimates.harm.as_ref();
            // Turn the nose at the candidate, for the seeker to hold it.
            if let Some(emitter) = cue.and_then(|c| {
                c.emitters
                    .iter()
                    .find(|e| Some(e.id) == c.candidate)
                    .or(c.emitters.first())
            }) {
                self.point = Some(emitter.position);
            }
            let line = cue.map(|c| {
                format!(
                    "harm cue candidate {} emitters {} cued {} readiness {:?}",
                    c.candidate
                        .map_or_else(|| "none".into(), |id| format!("{id:#x}")),
                    c.emitters.len(),
                    if c.cued { "yes" } else { "no" },
                    readout.estimates.readiness
                )
            });
            if line.is_some() && line != self.cue_line {
                self.cue_line.clone_from(&line);
                self.say(line.unwrap_or_default());
            }
            if let Some(candidate) = cue.filter(|c| !c.cued).and_then(|c| c.candidate)
                && readout.estimates.readiness == Readiness::Ready
            {
                self.fired = true;
                self.held_until = Some(now + PRESS);
                self.say(format!("fire {MISSILE} at {candidate:#x}"));
            }
        }
    }

    /// The strike role's commands (slice ST-K): A-G, Ctrl+X, the Mk 82s,
    /// Ctrl+M until the bomb sight says LVL; then one line with what it set.
    /// It flies level at the designated object.
    fn strike(
        &mut self,
        now: Duration,
        since: Duration,
        readout: &CockpitReadout,
        frame: &ClientFrame,
        controls: &mut Controls,
    ) {
        if self.radar_at.is_none() && since >= Duration::from_secs(2) {
            self.radar_at = Some(now);
            controls
                .commands
                .push(SeatCommand::Combat(Command::ToggleAirGround));
            self.say("A-G radar on".into());
        }
        let designated = readout.targets.designated;
        if self.radar_at.is_some()
            && designated.is_none()
            && self
                .designate_at
                .is_none_or(|t| now.saturating_sub(t) >= Duration::from_secs(2))
            && since >= Duration::from_secs(4)
        {
            self.designate_at = Some(now);
            controls
                .commands
                .push(SeatCommand::Combat(Command::NearestGround));
        }
        if let Some(id) = designated {
            self.point = frame
                .picture
                .targets
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.position)
                .or_else(|| readout.targets.display.as_ref().map(|t| t.position))
                .or(self.point);
        }
        let station = usize::from(readout.stores.selected);
        let selected = frame
            .config
            .stations
            .get(station)
            .map(|s| s.weapon.source.as_str());
        let ready = |t: Option<Duration>| t.is_none_or(|t| now.saturating_sub(t) >= PRESS * 2);
        if selected != Some(STRIKE_BOMB) || !readout.stores.armed {
            if ready(self.stepped_at) {
                self.stepped_at = Some(now);
                controls
                    .commands
                    .push(SeatCommand::CycleWeapon { forward: true });
            }
            return;
        }
        // The bomb role drops in CCIP: a press a second long every four
        // seconds releases one Mk 82 a press.
        if self.role == Role::Bomb {
            controls.trigger = since.as_secs().is_multiple_of(4);
            return;
        }
        let mode = readout.bomb_sight.as_ref().map(|b| b.mode);
        if mode != Some(DeliveryMode::Level) {
            if mode.is_some() && ready(self.mode_at) {
                self.mode_at = Some(now);
                controls
                    .commands
                    .push(SeatCommand::Combat(Command::ToggleSeekerMode));
            }
            return;
        }
        if let (Some(target), false) = (designated, self.announced) {
            self.announced = true;
            self.say(format!(
                "strike set target {target:#x} station {station} mode LVL laser {} rounds {}",
                hand_words(readout.laser.and_then(|l| l.manual)),
                readout.stores.rounds(station)
            ));
        }
    }

    /// The watching role's line: what the cockpit holds.
    fn cockpit(&mut self, readout: &CockpitReadout) {
        let station = usize::from(readout.stores.selected);
        self.say(format!(
            "cockpit designated {} station {station} mode {} laser {} rounds {}",
            readout
                .targets
                .designated
                .map_or_else(|| "none".into(), |t| format!("{t:#x}")),
            readout
                .bomb_sight
                .as_ref()
                .map_or("none", |b| b.mode.label()),
            hand_words(readout.laser.and_then(|l| l.manual)),
            readout.stores.rounds(station)
        ));
    }

    /// The periodic line: the laser and the spots, and the weapons in
    /// flight.
    fn report(&mut self, readout: &CockpitReadout) {
        if self.role == Role::Watch {
            self.cockpit(readout);
            return;
        }
        let Some(laser) = readout.laser else {
            self.say("laser none".into());
            return;
        };
        match (self.role, laser.spot) {
            (Role::Lase, Some(spot)) if spot.own => {
                let line = format!("laser {:?} spot {}", laser.state, spot_words(&spot));
                self.say(line);
            }
            (Role::Buddy, Some(spot)) if !spot.own => {
                let line = format!("buddy spot {}", spot_words(&spot));
                self.say(line);
            }
            _ => {
                let state = if laser.state == LaserState::Masked {
                    format!("Masked {:?}", laser.mask)
                } else {
                    format!("{:?}", laser.state)
                };
                self.say(format!(
                    "laser {state} no spot range {:.0} reach {:.0}",
                    laser.range_ft, laser.limit_ft
                ));
            }
        }
        for w in readout.weapon_tti.clone() {
            self.say(format!(
                "weapon {} {:?} guiding {} tti {}",
                w.projectile,
                w.kind,
                if w.guiding { "yes" } else { "no" },
                w.seconds
                    .map_or_else(|| "none".into(), |s| format!("{s:.1}"))
            ));
        }
    }

    /// Another plane's bombs and missiles, once each, as the picture draws
    /// them.
    fn see(&mut self, frame: &ClientFrame) {
        let own = frame.plane.0;
        for p in &frame.picture.projectiles {
            if p.gun || p.owner == own || !self.seen.insert(p.id) {
                continue;
            }
            self.lines.push(format!(
                "a2g: saw {} {} from plane {} target {}",
                p.weapon,
                p.id,
                p.owner,
                p.target
                    .map_or_else(|| "none".into(), |t| format!("{t:#x}"))
            ));
        }
    }

    /// Level at the seating's altitude at the point: straight at it, or for
    /// the lasing bot around it once near; `outbound`, straight away from
    /// it. Straight on with no point, or over it.
    fn steer(
        &mut self,
        flight: &flight::State,
        pilot: &mut ScriptedPilot,
        outbound: bool,
    ) -> Controls {
        let altitude = *pilot.altitude.get_or_insert(flight.position[1]);
        let position = flight.position;
        let forward = [flight.yaw.sin(), 0., flight.yaw.cos()];
        let ahead = [
            position[0] + forward[0] * 20_000.,
            altitude,
            position[2] + forward[2] * 20_000.,
        ];
        let aim = match self.point {
            None => ahead,
            Some(point) => {
                let flat = sub([point[0], position[1], point[2]], position);
                let range = length(flat);
                if outbound {
                    let away = flat.map(|v| -v / range.max(1.));
                    [
                        position[0] + away[0] * 20_000.,
                        altitude,
                        position[2] + away[2] * 20_000.,
                    ]
                } else if matches!(self.role, Role::Buddy | Role::Strike | Role::Bomb)
                    && range < OVERHEAD_FT
                {
                    ahead
                } else if self.role == Role::Lase && range < ORBIT_FT * 1.5 {
                    // A point on the circle a quarter turn on: a left orbit.
                    let toward = flat.map(|v| v / range.max(1.));
                    let left = [toward[2], 0., -toward[0]];
                    [
                        point[0] - toward[0] * ORBIT_FT + left[0] * ORBIT_FT,
                        altitude,
                        point[2] - toward[2] * ORBIT_FT + left[2] * ORBIT_FT,
                    ]
                } else {
                    [point[0], altitude, point[2]]
                }
            }
        };
        let (input, _) = pilot.steer(flight, position, aim);
        Controls {
            pilot: input,
            ..Controls::default()
        }
    }
}
