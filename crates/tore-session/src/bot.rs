//! The headless bot: the client session with a scripted pilot, for the
//! simulator matrix (D10) and the LAN smoke test (D11). See
//! docs/ARCHITECTURE.md, "The client session".
//!
//! The pilot flies straight and level, then turns, in a 40-second cycle,
//! holding its seating altitude; when an aircraft of the other side is within
//! 3 nautical miles it turns towards it, and it fires short gun bursts at
//! an aircraft of the other side within 5,000 feet and 6 degrees of where it
//! is going.
//! It reads only what a player's game has: its predicted flight and the
//! frame's picture.
//!
//! In the lobby (slice EF4) the bot plays as a game with no lobby screen:
//! the client's automatic ready takes the slot asked for (or the first free
//! one) with the standard loadout and marks ready, after each return to the
//! lobby and each mission change too. A bot that is the King
//! ([`Bot::start_when_ready`]) starts the mission as soon as every player
//! holding a slot is ready.

use crate::client::{Client, ClientFrame, Controls};
use crate::wire::messages::RosterPlane;
use std::collections::BTreeSet;
use std::f64::consts::{PI, TAU};
use std::time::Duration;
use tore_sim::flight::{self, PilotInput};
use tore_world::snapshot::RenderSnapshot;

/// The cycle: straight, a left turn, straight, a right turn, 10 s each.
const LEG: Duration = Duration::from_secs(10);
/// An enemy this close is chased, feet (3 nm).
const CHASE_FEET: f64 = 3. * 6076.;
/// The gun fires at an enemy this close, feet, and this near the flight
/// path, radians: wide enough that the scripted steering, which never holds a
/// tight aim, still fires in a fight.
const GUN_FEET: f64 = 5000.;
const GUN_ANGLE: f64 = 6. * PI / 180.;
/// A burst, and the pause after it.
const BURST: Duration = Duration::from_millis(400);
const PAUSE: Duration = Duration::from_millis(800);
/// The bot asks for a frame this often.
const FRAME_EVERY: Duration = Duration::from_millis(33);

fn wrap(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn heading(v: [f64; 3]) -> f64 {
    v[0].atan2(v[2])
}

/// The scripted pilot.
#[derive(Clone, Debug, Default)]
pub struct ScriptedPilot {
    started: Option<Duration>,
    altitude: Option<f64>,
    burst_since: Option<Duration>,
    /// Gun bursts begun.
    pub bursts: u64,
}

impl ScriptedPilot {
    /// A pilot that starts its cycle when first asked.
    pub fn new() -> Self {
        Self::default()
    }

    /// The controls at `now` for `flight`, chasing and firing at the
    /// aircraft of `picture` that `enemy` says are the other side's.
    pub fn controls(
        &mut self,
        now: Duration,
        flight: &flight::State,
        picture: Option<&RenderSnapshot>,
        enemy: &dyn Fn(u32) -> bool,
    ) -> Controls {
        let started = *self.started.get_or_insert(now);
        let hold = *self
            .altitude
            .get_or_insert(flight.position[1].clamp(3000., 30_000.));
        let course = heading(flight.velocity);
        let speed = flight.velocity.iter().map(|v| v * v).sum::<f64>().sqrt();
        let forward: [f64; 3] = if speed > 1. {
            flight.velocity.map(|v| v / speed)
        } else {
            [course.sin(), 0., course.cos()]
        };
        let mut target_heading = None;
        let mut target_altitude = hold;
        let mut fire = false;
        if let Some(picture) = picture {
            let mut nearest: Option<(f64, [f64; 3])> = None;
            for pose in picture.targets.iter().filter(|p| p.aircraft.is_some()) {
                if pose.crashed || pose.damage.hp <= 0 {
                    continue;
                }
                let d: [f64; 3] = std::array::from_fn(|i| pose.position[i] - flight.position[i]);
                let distance = d.iter().map(|v| v * v).sum::<f64>().sqrt();
                if enemy(pose.id) && distance < GUN_FEET && distance > 1. {
                    let cos = (0..3).map(|i| d[i] * forward[i]).sum::<f64>() / distance;
                    if cos.clamp(-1., 1.).acos() < GUN_ANGLE {
                        fire = true;
                    }
                }
                if enemy(pose.id)
                    && distance < CHASE_FEET
                    && nearest.is_none_or(|(n, _)| distance < n)
                {
                    nearest = Some((distance, d));
                }
            }
            if let Some((_, d)) = nearest {
                target_heading = Some(d[0].atan2(d[2]));
                target_altitude = (flight.position[1] + d[1]).clamp(3000., 30_000.);
            }
        }
        // The cycle's turn when chasing nothing.
        let leg = (now.saturating_sub(started).as_secs_f64() / LEG.as_secs_f64()) as u64 % 4;
        let bank_target = match target_heading {
            Some(want) => (wrap(want - course) * 1.5).clamp(-1., 1.),
            None => match leg {
                1 => -0.8,
                3 => 0.8,
                _ => 0.,
            },
        };
        let roll = ((bank_target - flight.bank) * 1.2).clamp(-0.6, 0.6);
        let climb = ((target_altitude - flight.position[1]) * 0.15).clamp(-80., 80.);
        let pull = (1. / flight.bank.cos().abs().max(0.3) - 1.) * 0.25;
        let pitch = ((climb - flight.vertical_speed) * 0.01 + pull).clamp(-0.4, 0.6);
        // Short bursts while something is ahead.
        let trigger = match self.burst_since {
            Some(since) if now.saturating_sub(since) < BURST => true,
            Some(since) if now.saturating_sub(since) < BURST + PAUSE => false,
            _ if fire => {
                self.burst_since = Some(now);
                self.bursts += 1;
                true
            }
            _ => {
                self.burst_since = None;
                false
            }
        };
        Controls {
            pilot: PilotInput {
                pitch,
                roll,
                ..PilotInput::default()
            },
            trigger,
            ..Controls::default()
        }
    }
}

/// The planes the roster puts on the other side from the client's own.
pub fn enemies(client: &Client) -> BTreeSet<u32> {
    let (Some(roster), Some((_, own))) = (client.roster(), client.seat()) else {
        return BTreeSet::new();
    };
    let side = |id: u32| {
        roster
            .planes
            .iter()
            .find(|p: &&RosterPlane| p.id == id)
            .map(|p| p.wing.side)
    };
    let Some(mine) = side(own.0) else {
        return BTreeSet::new();
    };
    roster
        .planes
        .iter()
        .filter(|p| p.wing.side != mine)
        .map(|p| p.id)
        .collect()
}

/// A client with the scripted pilot.
pub struct Bot {
    pub client: Client,
    pub pilot: ScriptedPilot,
    last_frame: Option<Duration>,
    picture: Option<RenderSnapshot>,
    /// Frames drawn.
    pub frames: u64,
    /// As the King, start the mission once every player holding a slot is
    /// ready.
    pub start_when_ready: bool,
    /// The lobby state the last Start was asked for.
    asked: Option<crate::wire::messages::LobbyState>,
}

impl Bot {
    /// The bot for a client that has started joining.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            pilot: ScriptedPilot::new(),
            last_frame: None,
            picture: None,
            frames: 0,
            start_when_ready: false,
            asked: None,
        }
    }

    /// The King's start, when every player holding a slot is ready and the
    /// lobby has changed since the last time it asked.
    fn start_if_ready(&mut self) {
        if !self.start_when_ready {
            return;
        }
        let Some(lobby) = self.client.lobby() else {
            return;
        };
        if lobby.is_king()
            && lobby.phase == crate::wire::messages::LobbyPhase::Lobby
            && lobby.all_ready()
            && self.asked.as_ref() != Some(lobby)
        {
            self.asked = Some(lobby.clone());
            self.client.start_mission();
        }
    }

    /// One update at `now`: a frame when due, the pilot's controls, the
    /// client's update. Returns the frame when one was drawn.
    pub fn update(&mut self, now: Duration) -> Option<ClientFrame> {
        let mut drawn = None;
        if self
            .last_frame
            .is_none_or(|last| now.saturating_sub(last) >= FRAME_EVERY)
        {
            self.last_frame = Some(now);
            if let Some(frame) = self.client.frame(now) {
                self.frames += 1;
                self.picture = Some(frame.picture.clone());
                drawn = Some(frame);
            }
        }
        let controls = match self.client.prediction() {
            Some(prediction) => {
                let flight = prediction.plane().flight.clone();
                let picture = self.picture.take();
                let enemies = enemies(&self.client);
                let controls = self
                    .pilot
                    .controls(now, &flight, picture.as_ref(), &|id| enemies.contains(&id));
                self.picture = picture;
                controls
            }
            None => Controls::default(),
        };
        self.client.update(now, &controls);
        self.start_if_ready();
        drawn
    }
}
