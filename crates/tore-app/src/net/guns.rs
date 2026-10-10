//! The gun rounds a networked client draws: its own at once from its trigger
//! and its predicted aircraft, and every other aircraft's from the host's gun
//! burst events (slice D8c; docs/ARCHITECTURE.md, "Hits and lag compensation").
//!
//! The host sends a burst as an event (shooter, gun station, first tick, and
//! the burst again with its length once it is over), never its rounds. The
//! rounds are made again here with the simulation's own rules
//! ([`tore_sim::combat::gun_round`]): the muzzle, the spread, the launch speed,
//! the way a round flies and the cadence the gun releases them on. They are
//! cosmetic. They hit nothing on the client, and the host decides every hit.
//!
//! Nothing here changes the simulation. The rounds are added to the picture as
//! ordinary [`ProjectilePose`]s with `gun: true` and the tracer flag, so the
//! drawing that already shows a host's rounds shows them.
//!
//! *Agent decisions:* the spread of a round comes from a number only this
//! side knows (the host seeds it with the round's projectile number, which no
//! message carries), so a round's line can differ from the host's within the
//! gun's half-degree cone; another aircraft's tracer pattern runs on from the
//! rounds its earlier bursts made (every third round, as the host's does),
//! starting at the first burst this client has seen of that gun; other
//! aircraft's rounds are placed from the shooter's pose drawn now, taken back
//! along its velocity to the round's release tick; a burst the host has not
//! closed yet is drawn as running on, and the rounds it overran are taken
//! back when the closing event comes (the picture's delay keeps that from
//! showing in practice).
use crate::snapshot::{AircraftPose, ProjectilePose, RenderSnapshot};
use std::collections::{BTreeMap, VecDeque};
use tore_formats::aircraft::AircraftId;
use tore_session::wire::events::{ReceivedEvent, WireEvent};
use tore_sim::{
    attitude::{Basis, Vector},
    combat::{
        gun_round::{self, Cadence, Round},
        live::{self, Launcher, Readiness, Station},
    },
};
use tore_world::{readout::CockpitReadout, seats::SeatCommand};

/// Ticks a frame catches the rounds up by at most: a stall is not caught up
/// on.
const MAX_CATCH_UP: u64 = 24;
/// Rounds alive at once, the host's own cap.
const MAX_ROUNDS: usize = live::MAX_PROJECTILES;
/// Ticks of an open burst kept without its closing event (20 seconds): a
/// burst the host never closed does not fire for ever.
const OPEN_BURST_TICKS: u64 = 2_400;
/// How far back a burst's rounds are still made (2 seconds): older ones are
/// out of the air by now, whatever the gun.
const BACKLOG_TICKS: u64 = 240;
/// Ticks the shooter's own rounds are remembered for counting its rounds
/// left (4 seconds).
const FIRED_KEPT_TICKS: u64 = 480;

/// Projectile numbers for the rounds drawn here: far above the host's, so
/// neither a camera that follows a missile nor the regenerated smoke mixes
/// one up with a host's.
const OWN_IDS: u32 = 0xF000_0000;
const OTHER_IDS: u32 = 0xE000_0000;

/// What a client frame brings that the rounds need.
pub struct Inputs<'a> {
    /// The newest predicted tick, which is the host tick the aircraft's last
    /// predicted step belongs to.
    pub tick: u64,
    /// The host tick the picture shows.
    pub render_tick: f64,
    /// Whether the seat's trigger is held now.
    pub trigger: bool,
    /// The seat's plane.
    pub plane: u32,
    /// The seat's aircraft as drawn: position, nose and speed.
    pub own: Launcher,
    /// What the newest readout says of the selected station, when one has
    /// come.
    pub stores: Option<Stores>,
    /// The seat's loadout.
    pub config: &'a live::Configuration,
    /// The events released since the last frame.
    pub events: &'a [ReceivedEvent],
}

/// What a cockpit readout says of the weapon selected: the page the gun
/// fires from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stores {
    pub selected: usize,
    /// The readout's readiness for the selected weapon is Ready: armed, with
    /// rounds, nothing in the way.
    pub ready: bool,
    /// Rounds at the selected station.
    pub rounds: u16,
    /// The host tick the readout shows.
    pub tick: u64,
}

impl Stores {
    /// The weapon page of `readout`.
    pub fn of(readout: &CockpitReadout) -> Self {
        let selected = readout.stores.selected();
        Self {
            selected,
            ready: readout.estimates.readiness == Readiness::Ready,
            rounds: readout.stores.rounds(selected),
            tick: readout.tick,
        }
    }
}

/// The seat's trigger as the host reads it: the keyboard's Space key, which
/// arrives as a seat command (`TriggerKey`, and `ReleaseTrigger` to let go),
/// and the controller's button, which is the controls' trigger. Both are
/// held by the world's own rule ([`crate::combat::FireInput`]), so the
/// trigger this says is held is the one the host's step will find held.
#[derive(Default)]
pub struct Fire {
    key: crate::combat::FireInput,
    pad: crate::combat::FireInput,
}

impl Fire {
    /// One turn's commands, in order, and the controller button; whether the
    /// trigger is held after them.
    pub fn turn(&mut self, commands: &[SeatCommand], pad: bool) -> bool {
        for command in commands {
            match *command {
                SeatCommand::TriggerKey {
                    down,
                    repeat,
                    blocked,
                } => self.key.space(down, repeat, blocked),
                SeatCommand::ReleaseTrigger => {
                    self.key.cancel();
                    self.pad.cancel();
                }
                _ => {}
            }
        }
        self.pad.space(pad, false, false);
        self.key.held || self.pad.held
    }
}

/// What the client knows of the mission.
pub struct Around<'a> {
    /// The mission's ground height by x and z.
    pub ground: &'a dyn Fn(f64, f64) -> f64,
    /// The stations of an aircraft type's usual loadout.
    pub stations: &'a dyn Fn(AircraftId) -> &'a [Station],
}

/// A gun burst of another aircraft, as its events tell it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Burst {
    shooter: u32,
    station: usize,
    /// The host tick of its first round.
    first: u64,
    /// The host tick of its last round, once the host has closed it.
    last: Option<u64>,
    /// The rounds the gun had let go before it, which sets its tracers.
    base: u64,
    /// The tick of each round made so far.
    made: Vec<u64>,
}

/// A round of another aircraft, and the burst and tick it was let go at.
struct Other {
    /// The round's own number, which stays with it as the rounds before it
    /// leave the air (a recorded flight keeps a round's identity).
    serial: u32,
    shooter: u32,
    station: usize,
    burst: u64,
    release: u64,
    round: Round,
}

/// The rounds a client draws.
#[derive(Default)]
pub struct Guns {
    /// The seat's trigger schedule for its selected gun.
    cadence: Cadence,
    /// The station `cadence` is for.
    station: Option<usize>,
    /// The seat's rounds, each with a number that stays with it while the
    /// rounds before it leave the air.
    own: Vec<(u32, Round)>,
    /// The number the seat's next round gets.
    own_serial: u32,
    /// The tick the seat's rounds have been made to.
    own_tick: u64,
    /// The ticks the seat's own rounds left, for counting the rounds it has
    /// left against a readout older than they are.
    fired: VecDeque<u64>,
    bursts: Vec<Burst>,
    /// The rounds each other aircraft's gun has let go, by shooter and station,
    /// as far as its bursts have told.
    ordinals: BTreeMap<(u32, usize), u64>,
    others: Vec<Other>,
    /// The host tick other aircraft's rounds have been made to.
    others_tick: u64,
    next_id: u32,
    /// The number the next round of another aircraft gets.
    next_serial: u32,
}

impl Guns {
    /// Makes and flies the rounds up to `input`'s ticks and adds every round
    /// alive to `picture`.
    pub fn step(&mut self, input: &Inputs<'_>, around: &Around<'_>, picture: &mut RenderSnapshot) {
        let own_events = |event: &&ReceivedEvent| matches!(&event.event, WireEvent::GunBurst { shooter, .. } if *shooter != input.plane);
        for received in input.events.iter().filter(own_events) {
            let WireEvent::GunBurst {
                shooter,
                station,
                length,
            } = &received.event
            else {
                continue;
            };
            self.note_burst(
                *shooter,
                usize::from(*station),
                u64::from(received.tick),
                *length,
            );
        }
        self.step_own(input, around);
        self.step_others(input, around, picture);

        let pose = |id: u32, owner: u32, round: &Round| ProjectilePose {
            id,
            owner,
            weapon: round.weapon.clone(),
            shape: None,
            gun: true,
            tracer: round.tracer,
            position: round.position,
            previous: round.previous,
            direction: round.direction,
            target: None,
            incoming: false,
            speed_f8: round.speed_f8,
        };
        // A round keeps its number for its whole flight, so whatever reads
        // the picture (the muzzle flashes) sees a new number only for a new
        // round.
        let own = self
            .own
            .iter()
            .map(|(serial, round)| pose(own_id(*serial), input.plane, round));
        let others = self
            .others
            .iter()
            .map(|other| pose(other_id(other.serial), other.shooter, &other.round));
        picture.projectiles.extend(own.chain(others));
    }

    /// A recorded flight's rounds (docs/ARCHITECTURE.md, "Converting a
    /// capture into a replay"): every aircraft's bursts, the player's too,
    /// made and flown to host tick `tick`, one tick at a time. `events` are
    /// the gun burst events of the tick, `picture` holds every aircraft
    /// drawn at it (the player's in its targets), and the rounds are read
    /// with [`Guns::rounds`]. A replay has no trigger, so only bursts make
    /// rounds.
    pub fn step_recorded(
        &mut self,
        events: &[ReceivedEvent],
        tick: u64,
        around: &Around<'_>,
        picture: &RenderSnapshot,
    ) {
        for received in events {
            if let WireEvent::GunBurst {
                shooter,
                station,
                length,
            } = &received.event
            {
                self.note_burst(
                    *shooter,
                    usize::from(*station),
                    u64::from(received.tick),
                    *length,
                );
            }
        }
        self.step_others_to(tick, around, picture);
    }

    /// The rounds of other aircraft in the air as [`Guns::step_recorded`]
    /// left them: a number that stays with the round, its shooter, how many
    /// ticks ago it was let go (`tick` is the tick it was stepped to) and the
    /// round.
    pub fn rounds(&self, tick: u64) -> impl Iterator<Item = (u32, u32, u64, &Round)> {
        self.others.iter().map(move |other| {
            (
                other_id(other.serial),
                other.shooter,
                tick.saturating_sub(other.release),
                &other.round,
            )
        })
    }

    /// A burst event of another aircraft: it opens a burst, or closes the open
    /// one that began at the same tick (a closing event of a burst never
    /// opened here makes the whole burst).
    fn note_burst(&mut self, shooter: u32, station: usize, first: u64, length: Option<u32>) {
        let last = length.map(|n| first + u64::from(n.max(1)) - 1);
        match self
            .bursts
            .iter_mut()
            .find(|b| b.shooter == shooter && b.station == station && b.first == first)
        {
            Some(burst) => {
                burst.last = burst.last.or(last);
                // Rounds made while the burst was open that it never fired.
                if let Some(last) = burst.last {
                    burst.made.retain(|tick| *tick <= last);
                    self.ordinals
                        .insert((shooter, station), burst.base + burst.made.len() as u64);
                    self.others.retain(|other| {
                        !(other.shooter == shooter
                            && other.station == station
                            && other.burst == first
                            && other.release > last)
                    });
                }
            }
            None => self.bursts.push(Burst {
                shooter,
                station,
                first,
                last,
                base: self
                    .ordinals
                    .get(&(shooter, station))
                    .copied()
                    .unwrap_or_default(),
                made: Vec::new(),
            }),
        }
    }

    /// The seat's own rounds, from its trigger, up to `input.tick`.
    fn step_own(&mut self, input: &Inputs<'_>, around: &Around<'_>) {
        // The first frame, a restart and a long stall have nothing to catch
        // up on.
        if self.own_tick == 0 || input.tick < self.own_tick {
            self.own_tick = input.tick;
        }
        self.own_tick = self.own_tick.max(input.tick.saturating_sub(MAX_CATCH_UP));
        let selected = input.stores.map(|stores| stores.selected);
        if selected != self.station {
            self.cadence.discard();
            self.station = selected;
        }
        // The station's gun, when the weapon page has a gun selected.
        let gun = selected
            .and_then(|station| input.config.stations.get(station))
            .filter(|station| live::is_gun(&station.weapon));
        // Rounds left: the readout's, less what has left since it was made.
        let left = input.stores.map_or(0, |stores| {
            let since = self
                .fired
                .iter()
                .filter(|tick| **tick > stores.tick)
                .count();
            usize::from(stores.rounds).saturating_sub(since)
        });
        let ready = input.stores.is_some_and(|stores| stores.ready);
        for tick in self.own_tick + 1..=input.tick {
            let back = (input.tick - tick) as f64 / 120.;
            self.own
                .retain_mut(|(_, round)| round.step(tick, &around.ground));
            let Some(station) = gun else {
                continue;
            };
            let fired = self.cadence.step(
                &station.weapon,
                input.trigger && input.own.alive,
                ready && left > 0 && self.own.len() < MAX_ROUNDS,
                tick,
            );
            let Some(fired) = fired else { continue };
            self.fired.push_back(tick);
            let launcher = input.own;
            let position: Vector =
                std::array::from_fn(|i| launcher.position[i] - launcher.velocity[i] * back);
            let muzzle = muzzle(position, launcher.basis, station.mount);
            let seed = [self.next_id, input.plane, self.station.unwrap_or(0) as u32];
            self.next_id = self.next_id.wrapping_add(1);
            if let Some(mut round) = Round::release(
                &station.weapon,
                muzzle,
                launcher.basis.forward,
                launcher.speed_fps,
                seed,
                tick,
                fired.tracer,
            ) && round.step(tick, &around.ground)
            {
                self.own_serial = self.own_serial.wrapping_add(1);
                self.own.push((self.own_serial, round));
            }
        }
        self.own_tick = input.tick;
        while self
            .fired
            .front()
            .is_some_and(|tick| tick + FIRED_KEPT_TICKS < input.tick)
        {
            self.fired.pop_front();
        }
    }

    /// Other aircraft's rounds, from their bursts, up to the host tick the
    /// picture shows.
    fn step_others(&mut self, input: &Inputs<'_>, around: &Around<'_>, picture: &RenderSnapshot) {
        self.step_others_to(input.render_tick.max(0.) as u64, around, picture);
    }

    fn step_others_to(&mut self, now: u64, around: &Around<'_>, picture: &RenderSnapshot) {
        if self.others_tick == 0 || now < self.others_tick {
            self.others_tick = now;
        }
        self.others_tick = self.others_tick.max(now.saturating_sub(MAX_CATCH_UP));
        for tick in self.others_tick + 1..=now {
            self.others
                .retain_mut(|other| other.round.step(tick, &around.ground));
            self.make_others(tick, now, around, picture);
        }
        self.others_tick = now;
        // Bursts that are over, or were never closed, are done with.
        self.bursts.retain(|burst| match burst.last {
            Some(last) => last + BACKLOG_TICKS >= now,
            None => burst.first + OPEN_BURST_TICKS > now,
        });
    }

    /// The rounds of every burst due at host tick `tick`, flown on to `now`.
    fn make_others(&mut self, tick: u64, now: u64, around: &Around<'_>, picture: &RenderSnapshot) {
        let mut bursts = std::mem::take(&mut self.bursts);
        for burst in &mut bursts {
            let Some(pose) = picture.targets.iter().find(|pose| pose.id == burst.shooter) else {
                continue;
            };
            let Some(aircraft) = pose.aircraft else {
                continue;
            };
            let stations = (around.stations)(aircraft);
            let Some(station) = stations
                .get(burst.station)
                .filter(|station| live::is_gun(&station.weapon))
                .or_else(|| stations.iter().find(|s| live::is_gun(&s.weapon)))
            else {
                continue;
            };
            loop {
                let n = burst.made.len() as u64;
                let release = gun_round::release_tick(&station.weapon, burst.first, n);
                if release > tick || burst.last.is_some_and(|last| release > last) {
                    break;
                }
                burst.made.push(release);
                self.ordinals
                    .insert((burst.shooter, burst.station), burst.base + n + 1);
                if now - release > BACKLOG_TICKS || self.others.len() >= MAX_ROUNDS {
                    continue;
                }
                if let Some(round) = other_round(
                    pose,
                    station,
                    release,
                    now,
                    (burst.base + n).is_multiple_of(3),
                    around,
                    &mut self.next_id,
                ) {
                    self.next_serial = self.next_serial.wrapping_add(1);
                    self.others.push(Other {
                        serial: self.next_serial,
                        shooter: burst.shooter,
                        station: burst.station,
                        burst: burst.first,
                        release,
                        round,
                    });
                }
            }
        }
        self.bursts = bursts;
    }
}

/// The picture's number for the seat's round `serial`, in its own block.
fn own_id(serial: u32) -> u32 {
    OWN_IDS | (serial & !OWN_IDS)
}
/// The picture's number for another aircraft's round `serial`, below the
/// seat's block.
fn other_id(serial: u32) -> u32 {
    OTHER_IDS | (serial & 0x0FFF_FFFF)
}

/// One round of another aircraft, let go at `release` and flown on to `now`
/// from where the aircraft drawn at `now` was then.
fn other_round(
    pose: &AircraftPose,
    station: &Station,
    release: u64,
    now: u64,
    tracer: bool,
    around: &Around<'_>,
    next_id: &mut u32,
) -> Option<Round> {
    let back = (now - release) as f64 / 120.;
    let position: Vector = std::array::from_fn(|i| pose.position[i] - pose.velocity[i] * back);
    let [yaw, pitch, bank] = pose.attitude;
    let basis = Basis::new(yaw, pitch, bank);
    let speed = pose.velocity.iter().map(|v| v * v).sum::<f64>().sqrt();
    let seed = [*next_id, pose.id, 0];
    *next_id = next_id.wrapping_add(1);
    let mut round = Round::release(
        &station.weapon,
        muzzle(position, basis, station.mount),
        basis.forward,
        speed,
        seed,
        release,
        tracer,
    )?;
    for tick in release..=now {
        if !round.step(tick, &around.ground) {
            return None;
        }
    }
    Some(round)
}

/// Where a gun at `mount` (feet right, up and forward of the aircraft's
/// centre) leaves an aircraft at `position` facing `basis`.
fn muzzle(position: Vector, basis: Basis, mount: Vector) -> Vector {
    std::array::from_fn(|i| {
        position[i]
            + basis.right[i] * mount[0]
            + basis.up[i] * mount[1]
            + basis.forward[i] * mount[2]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::weapons::Weapon;
    use tore_session::host::{BURST_SLACK_TICKS, round_interval};
    use tore_sim::combat::live::{Ownship, OwnshipInput, State};

    /// Where the guns are, feet right, up and forward of the aircraft.
    const MOUNT: Vector = [2., -1.5, 14.];
    /// The shooter, an aircraft of the mission that is not the seat's.
    const SHOOTER: u32 = 7;
    const TICKS: u64 = 700;
    /// Ticks the picture is behind the host: 100 ms.
    const DELAY: u64 = 12;

    /// The synthetic import's cannon, its burst record changed to `burst`
    /// (rounds in a burst, rounds per game round, burst time).
    fn gun(burst: (u8, u8, u8)) -> Weapon {
        let resources = tore_world::test_support::resources::resources();
        let mut weapon = Weapon::parse("M61.JT", &resources["M61.JT"]).unwrap();
        weapon.burst.game_rounds_in_burst = burst.0;
        weapon.burst.actual_rounds_per_game = burst.1;
        weapon.burst.game_burst_t = burst.2;
        weapon
    }

    fn configuration(weapon: &Weapon) -> live::Configuration {
        let mut config = tore_world::test_support::combat_fixture(false)
            .own()
            .configuration()
            .clone();
        config.stations[0].weapon = weapon.clone();
        config.stations[0].mount = MOUNT;
        config.stations[0].count = 2_000;
        config
    }

    /// The aircraft at combat tick `tick`: a banking turn, so the nose, the
    /// muzzle and the speed all change while the gun fires.
    fn launcher(tick: u64) -> Launcher {
        let seconds = tick as f64 / 120.;
        let (yaw, pitch, bank) = (0.4 + 0.05 * seconds, 0.04, 0.3);
        let basis = Basis::new(yaw, pitch, bank);
        let speed: f64 = 520. + 10. * seconds;
        let velocity = basis.forward.map(|v| v * speed);
        Launcher {
            position: std::array::from_fn(|i| [1_000., 9_000., -4_000.][i] + velocity[i] * seconds),
            basis,
            speed_fps: speed,
            velocity,
            bay_ready: true,
            radar_power: true,
            radar: true,
            jammer: false,
            alive: true,
            body_present: true,
            controls: tore_sim::sensors::Controls::default(),
        }
    }

    /// What the host did: the rounds in flight after each tick's step, in the
    /// order they were released, and the ticks and tracers of the releases.
    struct Host {
        flying: Vec<Vec<live::Projectile>>,
        fired: Vec<(u64, bool)>,
    }

    fn host(weapon: &Weapon, held: &dyn Fn(u64) -> bool) -> Host {
        let mut state = State::open_mission();
        state
            .add_ownship(
                Ownship::new(
                    SHOOTER,
                    live::DEFAULT_OWNSHIP_SIDE,
                    configuration(weapon),
                    true,
                )
                .unwrap(),
            )
            .unwrap();
        let mut run = Host {
            flying: Vec::new(),
            fired: Vec::new(),
        };
        let mut seen = 0;
        for tick in 0..TICKS {
            assert_eq!(state.tick(), tick);
            state.step(
                &[OwnshipInput {
                    aircraft: SHOOTER,
                    held: held(tick),
                    launcher: launcher(tick),
                }],
                |_, _| -100_000.,
            );
            let mut projectiles = state.projectiles.clone();
            projectiles.sort_by_key(|p| p.id);
            let before = seen;
            for p in projectiles.iter().filter(|p| p.id >= before) {
                run.fired.push((tick, p.tracer));
                seen = p.id + 1;
            }
            run.flying.push(projectiles);
        }
        run
    }

    fn ready() -> Option<Stores> {
        Some(Stores {
            selected: 0,
            ready: true,
            rounds: 2_000,
            tick: 0,
        })
    }

    fn around<'a>(
        ground: &'a dyn Fn(f64, f64) -> f64,
        stations: &'a dyn Fn(AircraftId) -> &'a [Station],
    ) -> Around<'a> {
        Around { ground, stations }
    }

    fn shooter_pose(tick: u64) -> AircraftPose {
        let at = launcher(tick);
        let [yaw, pitch, bank] = at.basis.angles();
        AircraftPose {
            id: SHOOTER,
            aircraft: Some(AircraftId::F18),
            position: at.position,
            attitude: [yaw, pitch, bank],
            velocity: at.velocity,
            airborne: true,
            ..AircraftPose::default()
        }
    }

    fn burst_event(number: u16, tick: u64, length: Option<u32>) -> ReceivedEvent {
        ReceivedEvent {
            number,
            tick: tick as u32,
            event: WireEvent::GunBurst {
                shooter: SHOOTER,
                station: 0,
                length,
            },
        }
    }

    /// The bursts the host would announce for `fired`: runs of rounds with no
    /// gap longer than the round interval plus the slack, as combat's burst
    /// tracker cuts them, with the tick each closing event arrives at.
    fn announced(weapon: &Weapon, fired: &[(u64, bool)]) -> Vec<(u64, u32, u64)> {
        let gap = round_interval(weapon) + BURST_SLACK_TICKS;
        let mut bursts: Vec<(u64, u64)> = Vec::new();
        for (tick, _) in fired {
            match bursts.last_mut() {
                Some((_, last)) if tick - *last <= gap => *last = *tick,
                _ => bursts.push((*tick, *tick)),
            }
        }
        bursts
            .into_iter()
            .map(|(first, last)| (first, (last - first + 1) as u32, last + gap + 1))
            .collect()
    }

    fn dist(a: Vector, b: Vector) -> f64 {
        (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
    }

    /// A trigger pulled in three bursts, the last one long enough to run on
    /// through the gun's own breaks.
    fn pulls(tick: u64) -> bool {
        (30..90).contains(&tick) || (200..230).contains(&tick) || (400..640).contains(&tick)
    }

    /// The guns the tests fire: slow and fast, whole and fractional ticks
    /// between rounds, and a repeat deadline with a wait.
    const BURSTS: [(u8, u8, u8); 4] = [(1, 2, 1), (4, 3, 2), (7, 3, 1), (2, 5, 3)];

    /// The seat's own rounds, drawn at once from its trigger and its aircraft,
    /// are the host's: the same count, released on the same ticks, with the
    /// same tracers, and with the host's seed for the spread, the same line.
    #[test]
    fn the_seats_own_rounds_are_the_hosts() {
        for burst in BURSTS {
            let weapon = gun(burst);
            let config = configuration(&weapon);
            let host = host(&weapon, &pulls);
            assert!(host.fired.len() > 10, "{burst:?} fired {:?}", host.fired);
            let ground = |_: f64, _: f64| -100_000.;
            let stations = |_: AircraftId| &[][..];
            let around = around(&ground, &stations);
            let mut guns = Guns::default();
            let mut released = Vec::new();
            for tick in 0..TICKS {
                let mut picture = RenderSnapshot::default();
                let inputs = Inputs {
                    tick,
                    render_tick: tick as f64,
                    trigger: pulls(tick),
                    plane: SHOOTER,
                    own: launcher(tick),
                    stores: ready(),
                    config: &config,
                    events: &[],
                };
                // The first call only sets the clock.
                guns.step(&inputs, &around, &mut picture);
                let mine = &host.flying[tick as usize];
                assert_eq!(
                    picture.projectiles.len(),
                    mine.len(),
                    "{burst:?} tick {tick}: rounds in the air"
                );
                for (ours, theirs) in picture.projectiles.iter().zip(mine) {
                    assert!(ours.gun);
                    assert_eq!(ours.tracer, theirs.tracer, "{burst:?} tick {tick}");
                    assert!(
                        dist(ours.position, theirs.position) < 0.01,
                        "{burst:?} tick {tick}: {:?} against {:?}",
                        ours.position,
                        theirs.position
                    );
                    assert!(dist(ours.previous, theirs.previous) < 0.01);
                }
                if guns.fired.back() == Some(&tick) && released.last() != Some(&tick) {
                    released.push(tick);
                }
            }
            let ticks: Vec<u64> = host.fired.iter().map(|(tick, _)| *tick).collect();
            // Ticks of the first frames (tick 0 sets the clock) are not rounds.
            assert_eq!(released, ticks, "{burst:?}: release ticks");
        }
    }

    /// With a seed of its own for the spread (the host's is a projectile number
    /// only it knows) a round is never further from the host's than the gun's
    /// spread allows: half a degree between two lines in the cone, at most 26
    /// feet in a second of a 3,000 ft/s round.
    #[test]
    fn with_its_own_spread_a_round_is_within_the_cone_of_the_hosts() {
        let weapon = gun((4, 3, 2));
        let config = configuration(&weapon);
        let host = host(&weapon, &pulls);
        let ground = |_: f64, _: f64| -100_000.;
        let stations = |_: AircraftId| &[][..];
        let around = around(&ground, &stations);
        let mut guns = Guns {
            next_id: 100_000,
            ..Guns::default()
        };
        let mut worst: f64 = 0.;
        for tick in 0..TICKS {
            let mut picture = RenderSnapshot::default();
            guns.step(
                &Inputs {
                    tick,
                    render_tick: tick as f64,
                    trigger: pulls(tick),
                    plane: SHOOTER,
                    own: launcher(tick),
                    stores: ready(),
                    config: &config,
                    events: &[],
                },
                &around,
                &mut picture,
            );
            let mine = &host.flying[tick as usize];
            assert_eq!(picture.projectiles.len(), mine.len());
            for (ours, theirs) in picture.projectiles.iter().zip(mine) {
                // The first second of the round's flight.
                if theirs.age <= 120 {
                    worst = worst.max(dist(ours.position, theirs.position));
                }
            }
        }
        assert!(worst > 0.5, "a different spread must differ ({worst})");
        assert!(worst < 27., "{worst} ft");
    }

    /// Another aircraft's rounds, made from its burst events alone, are the
    /// host's too: the same count, the same release ticks and, for the same
    /// spread seed, the same positions.
    #[test]
    fn another_aircrafts_rounds_come_from_its_burst_events() {
        for burst in BURSTS {
            let weapon = gun(burst);
            let config = configuration(&weapon);
            let host = host(&weapon, &pulls);
            let bursts = announced(&weapon, &host.fired);
            assert!(bursts.len() >= 3, "{burst:?}: {bursts:?}");
            let ground = |_: f64, _: f64| -100_000.;
            let stations = |id: AircraftId| {
                if id == AircraftId::F18 {
                    &config.stations[..]
                } else {
                    &[][..]
                }
            };
            let around = around(&ground, &stations);
            // The seat is another plane altogether.
            let own = Some(Stores {
                rounds: 0,
                ready: false,
                ..ready().unwrap()
            });
            let mut guns = Guns::default();
            let mut number = 0;
            // The picture is `DELAY` ticks behind the host, as a client's is.
            for tick in DELAY..TICKS + DELAY {
                let shown = tick - DELAY;
                let mut events = Vec::new();
                for (first, length, closed) in &bursts {
                    // An event is handed over once it has arrived and the
                    // picture has reached its tick.
                    if tick == *first + DELAY {
                        number += 1;
                        events.push(burst_event(number, *first, None));
                    }
                    if tick == (*closed).max(*first + DELAY) {
                        number += 1;
                        events.push(burst_event(number, *first, Some(*length)));
                    }
                }
                let mut picture = RenderSnapshot {
                    targets: vec![shooter_pose(shown)],
                    ..RenderSnapshot::default()
                };
                guns.step(
                    &Inputs {
                        tick,
                        render_tick: shown as f64,
                        trigger: false,
                        plane: 3,
                        own: launcher(tick),
                        stores: own,
                        config: &config,
                        events: &events,
                    },
                    &around,
                    &mut picture,
                );
                let mine = &host.flying[shown as usize];
                assert_eq!(
                    picture.projectiles.len(),
                    mine.len(),
                    "{burst:?} tick {shown}: rounds in the air"
                );
                for (ours, theirs) in picture.projectiles.iter().zip(mine) {
                    assert!(ours.gun);
                    assert_eq!(ours.owner, SHOOTER);
                    assert_eq!(ours.tracer, theirs.tracer, "{burst:?} tick {shown}");
                    assert!(
                        dist(ours.position, theirs.position) < 0.01,
                        "{burst:?} tick {shown}: {:?} against {:?}",
                        ours.position,
                        theirs.position
                    );
                }
            }
        }
    }

    /// A burst the host has not closed yet is drawn as running on; the rounds
    /// that overran it vanish when the closing event comes.
    #[test]
    fn a_closing_event_takes_back_the_rounds_an_open_burst_overran() {
        let weapon = gun((4, 3, 2));
        let config = configuration(&weapon);
        let ground = |_: f64, _: f64| -100_000.;
        let stations = |_: AircraftId| &config.stations[..];
        let around = around(&ground, &stations);
        let mut guns = Guns::default();
        let live = |guns: &mut Guns, tick: u64, events: &[ReceivedEvent]| {
            let mut picture = RenderSnapshot {
                targets: vec![shooter_pose(tick)],
                ..RenderSnapshot::default()
            };
            guns.step(
                &Inputs {
                    tick,
                    render_tick: tick as f64,
                    trigger: false,
                    plane: 3,
                    own: launcher(tick),
                    stores: None,
                    config: &config,
                    events,
                },
                &around,
                &mut picture,
            );
            picture.projectiles.len()
        };
        live(&mut guns, 99, &[]);
        // Rounds every 5 ticks from 100: at 100, 105, 110, 115 and 120.
        assert_eq!(live(&mut guns, 120, &[burst_event(1, 100, None)]), 5);
        // Closed as ending at 110: those at 115 and 120 never were.
        assert_eq!(live(&mut guns, 121, &[burst_event(2, 100, Some(11))]), 3);
        assert_eq!(live(&mut guns, 140, &[]), 3);
    }

    /// The seat's own plane in the host's burst events is not drawn again, and
    /// a burst of another aircraft that arrives late (its picture is already
    /// past the first rounds) is made up to where its rounds would be now.
    #[test]
    fn the_trigger_is_the_space_key_or_the_controller_as_the_world_holds_it() {
        let key = |down, repeat, blocked| SeatCommand::TriggerKey {
            down,
            repeat,
            blocked,
        };
        let mut fire = Fire::default();
        assert!(!fire.turn(&[], false));
        // Space goes down, and stays held while it repeats.
        assert!(fire.turn(&[key(true, false, false)], false));
        assert!(fire.turn(&[key(true, true, false)], false));
        assert!(!fire.turn(&[key(false, false, false)], false));
        // A press while blocked (a menu, a lost window) fires nothing and
        // stays blocked until it is let go.
        assert!(!fire.turn(&[key(true, false, true)], false));
        assert!(!fire.turn(&[key(true, false, false)], false));
        assert!(!fire.turn(&[key(false, false, false)], false));
        // The controller's button holds it as long as it is down, and a
        // release command lets go of both.
        assert!(fire.turn(&[], true));
        assert!(!fire.turn(&[SeatCommand::ReleaseTrigger], true));
        assert!(!fire.turn(&[], true));
        assert!(!fire.turn(&[], false));
        assert!(fire.turn(&[key(true, false, false)], false));
        assert!(!fire.turn(&[SeatCommand::ReleaseTrigger], false));
    }

    #[test]
    fn the_seats_own_burst_is_not_drawn_twice_and_a_late_burst_catches_up() {
        let weapon = gun((4, 3, 2));
        let config = configuration(&weapon);
        let ground = |_: f64, _: f64| -100_000.;
        let stations = |_: AircraftId| &config.stations[..];
        let around = around(&ground, &stations);
        let mut guns = Guns::default();
        let events = [burst_event(1, 100, None), burst_event(2, 100, Some(40))];
        let mut picture = RenderSnapshot {
            targets: vec![shooter_pose(100)],
            ..RenderSnapshot::default()
        };
        let mut input = Inputs {
            tick: 99,
            render_tick: 99.,
            trigger: false,
            plane: SHOOTER,
            own: launcher(99),
            stores: None,
            config: &config,
            events: &[],
        };
        guns.step(&input, &around, &mut picture);
        input.tick = 140;
        input.render_tick = 140.;
        input.events = &events;
        guns.step(&input, &around, &mut picture);
        assert!(picture.projectiles.is_empty(), "the seat's own burst");

        // The same burst of another aircraft, seen 20 ticks after it began.
        input.plane = 3;
        let mut guns = Guns::default();
        input.tick = 99;
        input.render_tick = 99.;
        input.events = &[];
        guns.step(&input, &around, &mut picture);
        input.tick = 120;
        input.render_tick = 120.;
        input.events = &events;
        let mut picture = RenderSnapshot {
            targets: vec![shooter_pose(120)],
            ..RenderSnapshot::default()
        };
        guns.step(&input, &around, &mut picture);
        // A round every 5 ticks from tick 100 to tick 140, those from 100 to
        // 120 in the air: five of them.
        assert_eq!(picture.projectiles.len(), 5);
    }

    #[test]
    fn a_round_keeps_its_picture_number_while_the_rounds_before_it_leave() {
        let weapon = gun((4, 3, 2));
        let config = configuration(&weapon);
        let ground = |_: f64, _: f64| -100_000.;
        let stations = |_: AircraftId| &config.stations[..];
        let around = around(&ground, &stations);
        let mut guns = Guns::default();
        let events = [burst_event(1, 100, None), burst_event(2, 100, Some(40))];
        let mut seen: Vec<u32> = Vec::new();
        let mut gone: Vec<u32> = Vec::new();
        let mut previous: Vec<u32> = Vec::new();
        for tick in 99..2_000 {
            let input = Inputs {
                tick,
                render_tick: tick as f64,
                trigger: false,
                plane: 3,
                own: launcher(tick),
                stores: None,
                config: &config,
                events: if tick == 101 { &events } else { &[] },
            };
            let mut picture = RenderSnapshot {
                targets: vec![shooter_pose(tick)],
                ..RenderSnapshot::default()
            };
            guns.step(&input, &around, &mut picture);
            let ids: Vec<u32> = picture.projectiles.iter().map(|p| p.id).collect();
            for id in &ids {
                assert!(!gone.contains(id), "round {id:x} came back at tick {tick}");
                if !seen.contains(id) {
                    seen.push(*id);
                }
            }
            gone.extend(previous.iter().filter(|id| !ids.contains(id)));
            previous = ids;
        }
        // Every round of the burst had a number of its own, and all have left.
        let rounds = (0..)
            .take_while(|n| gun_round::release_tick(&weapon, 100, *n) <= 139)
            .count();
        assert_eq!(seen.len(), rounds);
        assert!(previous.is_empty() && gone.len() == rounds);
        assert_eq!(own_id(5), OWN_IDS + 5);
        assert_eq!(other_id(0x1234_5678), OTHER_IDS + 0x0234_5678);
    }
}
